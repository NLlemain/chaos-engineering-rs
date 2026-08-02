# Queue Failure Pack

These profiles wrap queue endpoints with the rootless directional TCP proxy. They cause real broker disconnects, connection cuts before later acknowledgements, and duplicated protocol frames. Application-level outcomes such as a Kafka rebalance or duplicate message depend on the client and broker configuration, so those profiles are marked experimental.
