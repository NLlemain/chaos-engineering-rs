# IoT Failure Pack

MQTT clients can be pointed at these proxy ports to test intermittent connectivity and corrupted downstream frames. The Linux packet-loss profile uses `tc` and therefore requires `CAP_NET_ADMIN`; the rootless profiles need no elevated privileges.
