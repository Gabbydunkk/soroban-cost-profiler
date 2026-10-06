# Call Boundaries: Host vs WASM

The `soroban-cost-profiler` distinguishes between native WASM execution boundaries and Soroban Host function boundaries. 

## WASM Boundaries
WASM boundaries (`EventType::Call` and `EventType::Return`) represent native function invocations inside the loaded WASM binary. These events map standard instruction calls and returns.

## Host Boundaries 
Host boundaries (`EventType::HostCall` and `EventType::HostReturn`) represent the transition from WASM execution into a native Rust Soroban environment function (such as `require_auth` or ledger interactions).

### Snapshots
When transitioning into a host function, the tracer snapshots the `soroban_env_host` budget (`get_cpu_insns_consumed` and `get_mem_bytes_consumed`). Upon returning to WASM, the difference in consumed budget is natively calculated and emitted as the cost for that specific `HostReturn` event.
