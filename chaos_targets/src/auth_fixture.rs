use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rcgen::{BasicConstraints, CertificateParams, IsCa, KeyPair};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    sync::{Arc, RwLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub trait Clock: Send + Sync {
    fn now(&self) -> SystemTime;
}

#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

#[derive(Debug)]
pub struct ManualClock {
    now: RwLock<SystemTime>,
}

impl ManualClock {
    pub fn new(now: SystemTime) -> Self {
        Self {
            now: RwLock::new(now),
        }
    }

    pub fn advance(&self, duration: Duration) {
        let mut now = self.now.write().expect("manual clock lock poisoned");
        *now += duration;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> SystemTime {
        *self.now.read().expect("manual clock lock poisoned")
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct JwksDocument {
    pub keys: Vec<Jwk>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Jwk {
    pub kid: String,
    pub kty: &'static str,
    #[serde(rename = "use")]
    pub usage: &'static str,
    pub alg: &'static str,
    pub crv: &'static str,
    pub x: String,
    pub y: String,
    pub x5c: Vec<String>,
    #[serde(rename = "x5t#S256")]
    pub thumbprint: String,
}

#[derive(Debug)]
struct SigningMaterial {
    kid: String,
    certificate_der: Vec<u8>,
    private_key_der: Vec<u8>,
    public_key: Vec<u8>,
}

pub struct RotatingJwks {
    clock: Arc<dyn Clock>,
    overlap: Duration,
    current: SigningMaterial,
    previous: Option<(SigningMaterial, SystemTime)>,
    generation: u64,
}

impl RotatingJwks {
    pub fn new(clock: Arc<dyn Clock>, overlap: Duration) -> Result<Self> {
        Ok(Self {
            current: generate_material(1)?,
            previous: None,
            clock,
            overlap,
            generation: 1,
        })
    }

    pub fn current_kid(&self) -> &str {
        &self.current.kid
    }

    pub fn current_private_key_der(&self) -> &[u8] {
        &self.current.private_key_der
    }

    pub fn rotate(&mut self) -> Result<String> {
        self.generation += 1;
        let replacement = generate_material(self.generation)?;
        let old = std::mem::replace(&mut self.current, replacement);
        self.previous = Some((old, self.clock.now() + self.overlap));
        Ok(self.current.kid.clone())
    }

    pub fn document(&self) -> JwksDocument {
        let mut keys = vec![jwk(&self.current)];
        if let Some((previous, expires)) = &self.previous {
            if self.clock.now() < *expires {
                keys.push(jwk(previous));
            }
        }
        JwksDocument { keys }
    }

    pub fn accepts_kid(&self, kid: &str) -> bool {
        self.document().keys.iter().any(|key| key.kid == kid)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenTimeState {
    Valid,
    NotYetValid,
    Expired,
}

pub fn validate_token_window(
    clock: &dyn Clock,
    not_before: SystemTime,
    expires: SystemTime,
) -> Result<TokenTimeState> {
    if expires <= not_before {
        bail!("token expiry must be later than not-before");
    }
    let now = clock.now();
    Ok(if now < not_before {
        TokenTimeState::NotYetValid
    } else if now >= expires {
        TokenTimeState::Expired
    } else {
        TokenTimeState::Valid
    })
}

fn generate_material(generation: u64) -> Result<SigningMaterial> {
    let key = KeyPair::generate().context("generate JWKS signing key")?;
    let private_key_der = key.serialize_der();
    let public_key = key.public_key_raw().to_vec();
    if public_key.len() != 65 || public_key[0] != 4 {
        bail!("JWKS fixture expected an uncompressed P-256 public key");
    }
    let mut params = CertificateParams::new(vec!["jwks.chaos.test".to_string()])
        .context("create JWKS certificate parameters")?;
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let certificate = params
        .self_signed(&key)
        .context("self-sign JWKS fixture certificate")?;
    let certificate_der = certificate.der().as_ref().to_vec();
    let digest = Sha256::digest(&certificate_der);
    let kid = format!(
        "chaos-{}-{}",
        generation,
        &URL_SAFE_NO_PAD.encode(digest)[..12]
    );
    Ok(SigningMaterial {
        kid,
        certificate_der,
        private_key_der,
        public_key,
    })
}

fn jwk(material: &SigningMaterial) -> Jwk {
    let digest = Sha256::digest(&material.certificate_der);
    Jwk {
        kid: material.kid.clone(),
        kty: "EC",
        usage: "sig",
        alg: "ES256",
        crv: "P-256",
        x: URL_SAFE_NO_PAD.encode(&material.public_key[1..33]),
        y: URL_SAFE_NO_PAD.encode(&material.public_key[33..65]),
        x5c: vec![base64::engine::general_purpose::STANDARD.encode(&material.certificate_der)],
        thumbprint: URL_SAFE_NO_PAD.encode(digest),
    }
}

pub fn unix_time(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn certificate_rotation_overlaps_then_retires_old_key() {
        let clock = Arc::new(ManualClock::new(unix_time(1_700_000_000)));
        let mut fixture = RotatingJwks::new(clock.clone(), Duration::from_secs(60)).unwrap();
        let old = fixture.current_kid().to_string();
        let old_key = fixture.current_private_key_der().to_vec();
        let new = fixture.rotate().unwrap();

        assert_ne!(old, new);
        assert_ne!(old_key, fixture.current_private_key_der());
        assert!(fixture.accepts_kid(&old));
        assert!(fixture.accepts_kid(&new));
        assert_eq!(fixture.document().keys.len(), 2);

        clock.advance(Duration::from_secs(61));
        assert!(!fixture.accepts_kid(&old));
        assert!(fixture.accepts_kid(&new));
        assert_eq!(fixture.document().keys.len(), 1);
    }

    #[test]
    fn controllable_clock_crosses_token_boundaries_without_sleeping() {
        let clock = ManualClock::new(unix_time(1_000));
        let not_before = unix_time(1_010);
        let expires = unix_time(1_020);
        assert_eq!(
            validate_token_window(&clock, not_before, expires).unwrap(),
            TokenTimeState::NotYetValid
        );
        clock.advance(Duration::from_secs(10));
        assert_eq!(
            validate_token_window(&clock, not_before, expires).unwrap(),
            TokenTimeState::Valid
        );
        clock.advance(Duration::from_secs(10));
        assert_eq!(
            validate_token_window(&clock, not_before, expires).unwrap(),
            TokenTimeState::Expired
        );
    }
}
