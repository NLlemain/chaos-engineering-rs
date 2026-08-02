# Authentication Failure Pack

These scenarios provide real local endpoints for expired certificates, incomplete certificate chains, unavailable JWKS documents, and OAuth refresh rejection. Point the client under test at the scenario's `listen` address. Replace example upstream URLs before running HTTP scenarios.

Clock skew and mutation of already-signed JWTs remain planned because they require an explicit process or token boundary; the catalog does not claim those faults are active yet.
