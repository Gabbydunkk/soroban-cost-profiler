use tracing::{debug, error, info, trace};
use crate::models::{EventType, TraceEvent};
use soroban_env_host::{Host, budget::AsBudget};

/// Hooks into the WASM execution engine to emit `TraceEvent`s.
pub struct ExecutionTracer {
    pub events: Vec<TraceEvent>,
    pub current_step_cost: u64,
    pub current_mem_cost: u64,
    pub sample_rate: u64,
    pub instruction_count: u64,
    pub instruction_ceiling: u64,
    
    // Snapshots of the host's budget
    pub host_snapshot_cpu: u64,
    pub host_snapshot_mem: u64,
}

impl Default for ExecutionTracer {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            current_step_cost: 0,
            current_mem_cost: 0,
            sample_rate: 100, // Default sample rate
            instruction_count: 0,
            instruction_ceiling: 100_000_000,
            host_snapshot_cpu: 0,
            host_snapshot_mem: 0,
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
    
    pub fn with_instruction_ceiling(mut self, ceiling: u64) -> Self {
        self.instruction_ceiling = ceiling;
        self
    }

    pub fn record_step(&mut self, pc: usize, cpu_cost: u64, mem_cost: u64) -> Result<(), &'static str> {
        trace!("Stepping at PC: {}, cpu: {}, mem: {}", pc, cpu_cost, mem_cost);
        self.instruction_count = self.instruction_count.saturating_add(1);
        if self.instruction_count > self.instruction_ceiling {
            error!("Instruction ceiling exceeded at PC: {}", pc);
            return Err("Instruction ceiling exceeded");
        }
        
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
        Ok(())
    }

    pub fn record_call(&mut self, pc: usize, cpu_cost: u64, mem_cost: u64) {
        debug!("WASM Call at PC: {}", pc);
        self.events.push(TraceEvent {
            pc,
            event_type: EventType::Call,
            cpu_cost,
            mem_cost,
        });
    }

    pub fn record_return(&mut self, pc: usize, cpu_cost: u64, mem_cost: u64) {
        debug!("WASM Return at PC: {}", pc);
        self.events.push(TraceEvent {
            pc,
            event_type: EventType::Return,
            cpu_cost,
            mem_cost,
        });
    }
    
    pub fn record_host_call(&mut self, pc: usize, host: &Host) {
        debug!("Host Call at PC: {}", pc);
        let budget = host.as_budget();
        self.host_snapshot_cpu = budget.get_cpu_insns_consumed().unwrap_or(0);
        self.host_snapshot_mem = budget.get_mem_bytes_consumed().unwrap_or(0);
        
        self.events.push(TraceEvent {
            pc,
            event_type: EventType::HostCall,
            cpu_cost: 0,
            mem_cost: 0,
        });
    }

    pub fn record_host_return(&mut self, pc: usize, host: &Host) {
        debug!("Host Return at PC: {}", pc);
        let budget = host.as_budget();
        let current_cpu = budget.get_cpu_insns_consumed().unwrap_or(0);
        let current_mem = budget.get_mem_bytes_consumed().unwrap_or(0);
        
        let diff_cpu = current_cpu.saturating_sub(self.host_snapshot_cpu);
        let diff_mem = current_mem.saturating_sub(self.host_snapshot_mem);

        self.events.push(TraceEvent {
            pc,
            event_type: EventType::HostReturn,
            cpu_cost: diff_cpu,
            mem_cost: diff_mem,
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
    info!("Loading WASM file from {}", path);
    let bytes = std::fs::read(path)?;
    // Issue 32: Reject malformed / non-Soroban WASM binaries gracefully
    if bytes.len() < 4 || &bytes[0..4] != b"\0asm" {
        error!("Invalid WASM signature for file: {}", path);
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid WASM signature",
        ));
    }
    Ok(bytes)
}

pub fn setup_engine() -> wasmi::Engine {
    let mut config = wasmi::Config::default();
    config.consume_fuel(true);
    wasmi::Engine::new(&config)
}

pub fn parse_module(engine: &wasmi::Engine, wasm_bytes: &[u8]) -> Result<wasmi::Module, wasmi::Error> {
    wasmi::Module::new(engine, wasm_bytes)
}

pub fn setup_mock_env() -> Host {
    Host::default()
}

#[tracing::instrument(skip(engine, store, module))]
pub fn instantiate_module(
    engine: &wasmi::Engine,
    store: &mut wasmi::Store<()>,
    module: &wasmi::Module,
) -> Result<wasmi::Instance, wasmi::Error> {
    info!("Instantiating WASM module");
    let linker = <wasmi::Linker<()>>::new(engine);
    linker.instantiate_and_start(store, module)
}

#[tracing::instrument(skip(store, instance, params, results))]
pub fn invoke_function(
    store: &mut wasmi::Store<()>,
    instance: &wasmi::Instance,
    func_name: &str,
    params: &[wasmi::Val],
    results: &mut [wasmi::Val],
) -> Result<(), wasmi::Error> {
    info!("Invoking function: {}", func_name);
    let func = instance
        .get_func(&mut *store, func_name)
        .ok_or_else(|| {
            error!("Function '{}' not found", func_name);
            wasmi::Error::new(format!("Function '{}' not found", func_name))
        })?;
    func.call(store, params, results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_step_sampling() {
        let mut tracer = ExecutionTracer::new().with_sample_rate(100);

        let _ = tracer.record_step(1, 50, 10);
        assert!(tracer.events.is_empty());

        let _ = tracer.record_step(2, 60, 20);
        assert_eq!(tracer.events.len(), 1);
        assert_eq!(tracer.events[0].pc, 2);
        assert_eq!(tracer.events[0].cpu_cost, 110);
        assert_eq!(tracer.events[0].mem_cost, 30);
        assert_eq!(tracer.events[0].event_type, EventType::Step);

        let _ = tracer.record_step(3, 40, 0);
        assert_eq!(tracer.events.len(), 1);
    }

    #[test]
    fn test_record_call_and_return() {
        let mut tracer = ExecutionTracer::new().with_sample_rate(100);

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
        let host = setup_mock_env();
        
        tracer.record_host_call(1, &host);
        
        let _ = host.as_budget().charge(
            soroban_env_host::xdr::ContractCostType::WasmInsnExec,
            Some(100)
        );

        tracer.record_host_return(2, &host);

        assert_eq!(tracer.events.len(), 2);
        assert_eq!(tracer.events[0].event_type, EventType::HostCall);
        assert_eq!(tracer.events[1].event_type, EventType::HostReturn);
        assert_eq!(tracer.events[1].cpu_cost, 0); 
        assert_eq!(tracer.events[1].mem_cost, 0);
    }

    #[test]
    fn test_instruction_ceiling() {
        let mut tracer = ExecutionTracer::new().with_instruction_ceiling(2);
        assert!(tracer.record_step(1, 10, 0).is_ok());
        assert!(tracer.record_step(2, 10, 0).is_ok());
        assert!(tracer.record_step(3, 10, 0).is_err());
    }
}

#[cfg(test)]
mod recursive_tests {
    use super::*;

    #[test]
    fn test_recursive_function_calls() {
        let mut tracer = ExecutionTracer::new().with_sample_rate(100);
        let depth = 1000;
        
        for i in 0..depth {
            tracer.record_call(i, 5, 2);
        }
        
        for i in (0..depth).rev() {
            tracer.record_return(i, 5, 2);
        }
        
        assert_eq!(tracer.events.len(), 2000);
        assert_eq!(tracer.events[0].event_type, EventType::Call);
        assert_eq!(tracer.events[1999].event_type, EventType::Return);
    }
}
