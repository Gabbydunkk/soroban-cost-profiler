use crate::models::{EventType, TraceEvent};

#[derive(Default)]
pub struct MockHost {
    // Add internal mock state here
}

impl MockHost {
    pub fn new() -> Self {
        Self {}
    }
}

/// Hooks into the WASM execution engine to emit `TraceEvent`s.
pub struct ExecutionTracer {
    pub events: Vec<TraceEvent>,
    pub current_step_cost: u64,
    pub current_mem_cost: u64,
    pub sample_rate: u64,
    // TODO: Add WASM engine hooks or host references here
}

impl Default for ExecutionTracer {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            current_step_cost: 0,
            current_mem_cost: 0,
            sample_rate: 100, // Default sample rate
        }
    }
}

impl ExecutionTracer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_sample_rate(mut self, sample_rate: u64) -> Self {
        self.sample_rate = sample_rate;
        self
    }

    pub fn record_step(&mut self, pc: usize, cpu_cost: u64, mem_cost: u64) {
        self.current_step_cost = self.current_step_cost.saturating_add(cpu_cost);
        self.current_mem_cost = self.current_mem_cost.saturating_add(mem_cost);
        if self.current_step_cost >= self.sample_rate {
            self.events.push(TraceEvent {
                pc,
                event_type: EventType::Step,
                cpu_cost: self.current_step_cost,
                mem_cost: self.current_mem_cost,
            });
            self.current_step_cost = 0;
            self.current_mem_cost = 0;
        }
    }

    pub fn record_call(&mut self, pc: usize, cpu_cost: u64, mem_cost: u64) {
        self.events.push(TraceEvent {
            pc,
            event_type: EventType::Call,
            cpu_cost,
            mem_cost,
        });
    }

    pub fn record_return(&mut self, pc: usize, cpu_cost: u64, mem_cost: u64) {
        self.events.push(TraceEvent {
            pc,
            event_type: EventType::Return,
            cpu_cost,
            mem_cost,
        });
    }

    pub fn record_host_call(&mut self, pc: usize, cpu_cost: u64, mem_cost: u64) {
        self.events.push(TraceEvent {
            pc,
            event_type: EventType::HostCall,
            cpu_cost,
            mem_cost,
        });
    }

    pub fn record_host_return(&mut self, pc: usize, cpu_cost: u64, mem_cost: u64) {
        self.events.push(TraceEvent {
            pc,
            event_type: EventType::HostReturn,
            cpu_cost,
            mem_cost,
        });
    }

    pub fn flush_trace(&mut self) -> Vec<TraceEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn trace(&mut self) -> Vec<TraceEvent> {
        self.events.clone()
    }
}

pub fn load_wasm_file(path: &str) -> std::io::Result<Vec<u8>> {
    std::fs::read(path)
}

pub fn setup_engine() -> wasmi::Engine {
    let mut config = wasmi::Config::default();
    config.consume_fuel(true);
    wasmi::Engine::new(&config)
}

pub fn parse_module(
    engine: &wasmi::Engine,
    wasm_bytes: &[u8],
) -> Result<wasmi::Module, wasmi::Error> {
    wasmi::Module::new(engine, wasm_bytes)
}

pub fn create_host() -> soroban_env_host::Host {
    soroban_env_host::Host::default()
}

pub fn instantiate_module(
    engine: &wasmi::Engine,
    store: &mut wasmi::Store<()>,
    module: &wasmi::Module,
) -> Result<wasmi::Instance, wasmi::Error> {
    let linker = <wasmi::Linker<()>>::new(engine);
    // TODO: Define host imports and add to linker here
    linker.instantiate_and_start(store, module)
}

pub fn invoke_function(
    store: &mut wasmi::Store<()>,
    instance: &wasmi::Instance,
    func_name: &str,
    params: &[wasmi::Val],
    results: &mut [wasmi::Val],
) -> Result<(), wasmi::Error> {
    let func = instance
        .get_func(&mut *store, func_name)
        .ok_or_else(|| wasmi::Error::new(format!("Function '{}' not found", func_name)))?;
    func.call(store, params, results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_step_sampling() {
        let mut tracer = ExecutionTracer::new().with_sample_rate(100);

        // Add 50 cost (no event should fire)
        tracer.record_step(1, 50, 10);
        assert!(tracer.events.is_empty());

        // Add 60 cost (total 110 >= 100, event should fire)
        tracer.record_step(2, 60, 20);
        assert_eq!(tracer.events.len(), 1);
        assert_eq!(tracer.events[0].pc, 2);
        assert_eq!(tracer.events[0].cpu_cost, 110);
        assert_eq!(tracer.events[0].mem_cost, 30);
        assert_eq!(tracer.events[0].event_type, EventType::Step);

        // Add 40 cost (no event)
        tracer.record_step(3, 40, 0);
        assert_eq!(tracer.events.len(), 1);
    }

    #[test]
    fn test_record_call_and_return() {
        let mut tracer = ExecutionTracer::new().with_sample_rate(100);

        // Call and return should bypass sampling
        tracer.record_call(1, 10, 5);
        tracer.record_return(2, 20, 10);

        assert_eq!(tracer.events.len(), 2);
        assert_eq!(tracer.events[0].event_type, EventType::Call);
        assert_eq!(tracer.events[0].cpu_cost, 10);
        assert_eq!(tracer.events[0].mem_cost, 5);
        assert_eq!(tracer.events[1].event_type, EventType::Return);
        assert_eq!(tracer.events[1].cpu_cost, 20);
        assert_eq!(tracer.events[1].mem_cost, 10);
    }

    #[test]
    fn test_record_host_call_and_return() {
        let mut tracer = ExecutionTracer::new().with_sample_rate(100);

        tracer.record_host_call(1, 5, 2);
        tracer.record_host_return(2, 8, 4);

        assert_eq!(tracer.events.len(), 2);
        assert_eq!(tracer.events[0].event_type, EventType::HostCall);
        assert_eq!(tracer.events[0].cpu_cost, 5);
        assert_eq!(tracer.events[0].mem_cost, 2);
        assert_eq!(tracer.events[1].event_type, EventType::HostReturn);
        assert_eq!(tracer.events[1].cpu_cost, 8);
        assert_eq!(tracer.events[1].mem_cost, 4);
    }
}
