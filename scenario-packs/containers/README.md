# Docker and Compose Failure Pack

`compose-pause.yaml` resolves the current container IDs for a Compose service, records every container's running and paused state, pauses the service, and restores only the state changed by the experiment. Change the service and Compose file before running it.

The same injector supports direct container IDs and the `pause`, `stop`, `kill`, and `restart` actions. Recovery uses the recorded IDs even if the Compose project later changes.
