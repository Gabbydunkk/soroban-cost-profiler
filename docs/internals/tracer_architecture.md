# Tracer Architecture

The execution tracer is designed to intercept WASM instruction boundaries using `wasmi` Engine Hooks. Because evaluating every single instruction adds massive overhead (a single Soroban transaction can run up to 100M instructions), the tracer employs a sampling mechanism.

## Instruction Metering

We leverage `wasmi`'s built-in fuel consumption (`consume_fuel(true)`) to measure computational cost natively. As the engine executes blocks of code, it deducts fuel. The profiler periodically reads this fuel consumption to attribute cost without needing to manually map every single instruction.

## The Event Buffer

`ExecutionTracer` maintains an internal vector of `TraceEvent` structures.
Instead of pushing an event for every WASM step, `record_step` aggregates `cpu_cost` into `current_step_cost`. When this accumulator hits the `sample_rate` threshold, a single `TraceEvent::Step` is pushed to the buffer, reducing memory overhead by several orders of magnitude.

Function boundaries (`TraceEvent::Call` and `TraceEvent::Return`) bypass the sampling filter completely. Capturing exact call stack boundaries is critical for attributing the sampled instruction costs to the correct parent function later in the pipeline.

## Soroban Host

The profiler instantiates a native `soroban_env_host::Host` to manage cross-contract calls and ledger state injections. During execution, any host function call that traps into the Soroban environment seamlessly proxies back to our instantiated `Host`.
