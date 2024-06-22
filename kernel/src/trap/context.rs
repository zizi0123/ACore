use riscv::register::sstatus;

#[repr(C)]
pub struct TrapContext {
    pub x: [usize; 32],   // 32 general purpose registers
    pub sstatus: sstatus::Sstatus, // previous privilege mode, etc.
    pub sepc: usize,      // 防止 trap 嵌套被覆盖
    pub kernel_satp: usize,
    pub kernel_sp: usize,    // the va of the current app's kernel stack sp in kernel space
    pub trap_handler: usize, // the va of the trap handler's entry in kernel space
}

impl TrapContext {
    // create a new TrapContext when a new app is initialized
    pub fn new(
        entry: usize, // entry point of the app after trap
        sp: usize, // the va of the current app's user stack sp
        kernel_satp: usize,
        kernel_sp: usize,
        trap_handler: usize,
    ) -> Self {
        let mut sstatus = sstatus::read();
        sstatus.set_spp(sstatus::SPP::User);
        let mut trap_context = Self {
            x: [0; 32],
            sstatus,
            sepc: entry,
            kernel_satp,
            kernel_sp,
            trap_handler,
        };
        trap_context.x[2] = sp;
        return trap_context;
    }
}
