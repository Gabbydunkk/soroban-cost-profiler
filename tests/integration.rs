use soroban_cost_profiler::tracer::{
    ExecutionTracer, ProfilerState, instantiate_module, parse_module, setup_engine, setup_mock_env,
};
use wasmi::Store;

#[test]
fn test_fixture_compile_and_trace() {
    let engine = setup_engine();
    let wasm_bytes = b"\0asm\x01\0\0\0"; // Minimal valid empty WASM for test
    let module = parse_module(&engine, wasm_bytes).expect("Failed to parse minimal WASM");
    let state = ProfilerState {
        tracer: ExecutionTracer::new(),
        host: setup_mock_env(),
        last_fuel: 0,
    };
    let mut store = Store::new(&engine, state);
    let _instance =
        instantiate_module(&engine, &mut store, &module).expect("Failed to instantiate");

    // Test the integration tracer flow
    let _ = store.data_mut().tracer.record_step(0, 10, 5);

    let events = store.data_mut().tracer.flush_trace();
    assert_eq!(events.len(), 0); // Didn't hit the default 100 sample rate
}
