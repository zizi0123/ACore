use riscv::register::sstatus::Sstatus;


const STACK_SIZE: usize = 4096 * 2;

#[repr(align(4096))]
struct Stack {
    data: [u8; STACK_SIZE],
}

impl Stack {
    fn get_sp(&self) -> usize {
        return self.data.as_ptr() as usize + STACK_SIZE
    }
}

static KERNEL_STACK: Stack = Stack { data: [0; STACK_SIZE] };
static USER_STACK: Stack = Stack { data: [0; STACK_SIZE] };

#[repr(C)]
pub struct TrapContext {
    pub x: [usize; 32],
    pub sstatus: Sstatus,
    pub sepc: usize,
}