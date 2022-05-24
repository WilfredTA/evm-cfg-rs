use crate::op::*;
#[derive(Clone, Debug, Copy)]
pub enum SymbolicStackValue {
    Data([u8; 256]),
    Unknown,
    Uninitialized
}

impl SymbolicStackValue {
    pub fn inner(&self) -> Option<&[u8; 256]> {
        if let SymbolicStackValue::Data(dat) = self {
            Some(dat)
        } else {
            None
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub struct SymbolicStackFrame {
    contents: SymbolicStackValue
}

impl SymbolicStackFrame {
    pub fn new() -> Self {
        Self {
            contents: SymbolicStackValue::Uninitialized
        }
    }

    pub fn peek(&self) -> SymbolicStackValue {
        self.contents.clone()
    }

    pub fn inner(&self) -> &SymbolicStackValue {
        &self.contents
    }
    pub fn new_with_unknown_val() -> Self {
        Self {
            contents: SymbolicStackValue::Unknown
        }
    }

    pub fn write(&mut self, val: &[u8]) {
        let mut data = [0u8; 256];
        data[..val.len()].copy_from_slice(val);
        self.contents = SymbolicStackValue::Data(data);
    }


}

#[derive(Debug, Clone)]
pub struct SymbolicStack {
    pub frames: [SymbolicStackFrame; 1024],
    pc: usize,
}

pub struct SymbolicStackCapture {
    pub frame_count: usize,
    pub pc: usize,
    pub vals: Box<Vec<SymbolicStackFrame>>
}


impl From<SymbolicStackCapture> for SymbolicStack {
    fn from(capture: SymbolicStackCapture) -> Self {
        let mut frames = [SymbolicStackFrame::new(); 1024];
        frames[..capture.vals.len()].copy_from_slice(&capture.vals);
        Self {
            frames ,
            pc: capture.pc,
        }
    }
}
impl SymbolicStack {
    pub fn capture(&self) -> SymbolicStackCapture {
        let stack = self;
        let mut i = 0;
        let mut len = 0;
        loop {
            match stack.frames[i].inner() {
                &SymbolicStackValue::Uninitialized => {
                    len = i + 1;
                    break;
                },
                _ => {
                    i += 1;
                }
            }
        }

        let vals = Box::new(stack.frames[0..len].to_vec());
        SymbolicStackCapture {
            frame_count: len,
            pc: stack.pc,
            vals,
        }
    }
    pub fn peek(&self) -> SymbolicStackValue {
        self.frames[self.pc].peek()
    }
    pub fn new() -> Self {
        Self {
            pc: 0,
            frames: [SymbolicStackFrame::new(); 1024]
        }
    }
    pub fn pop(&mut self) -> SymbolicStackFrame {
        self.pc -= 1;
        self.frames[self.pc + 1].clone()
    }

    pub fn push(&mut self, val: Option<&[u8]>) {
        self.pc += 1;
        if let Some(val) = val {
            self.frames[self.pc].write(val);
        } else {
            self.frames[self.pc] = SymbolicStackFrame::new_with_unknown_val();
        }

    }

    pub fn execute(&mut self, op: &Operation, code: &[u8]) {
        match op.category() {
            OpType::And => {
                // frames[self.pc] & frames[self.pc - 1]
                let top = self.pop();
                let top = top.inner().inner();
                let second = self.pop();
                let second = second.inner().inner();
                if top.is_none() || second.is_none() {
                    self.push(Some([0u8;256].as_slice()));
                    return;
                }
                let top = top.unwrap();
                let second = second.unwrap();
                let mut res = [0u8; 256];
                let mut i = 0;
                while i < 256 {
                    let byte_res = top[i] & second[i];
                    res[i] = byte_res;
                    i += 1;
                }
                self.push(Some(res.as_slice()));
            },
            OpType::Pop => {
                self.pop();
            },
            OpType::Push => {
                println!("OP IN STACK EXECUTE: {:?}", op);
                let push_byte_len = op.arg_size as usize;
                let start_loc = op.pc.unwrap().as_usize() + 1;
                let end_loc = start_loc + push_byte_len;
                let push_bytes = &code[start_loc..end_loc];
                self.push(Some(push_bytes));
            },
            OpType::Swap => {
                let swap_frame_count = op.rm_stack_count as usize;
                let top_addr = self.pc;
                let swap_addr = (self.pc - swap_frame_count) + 1;
                let temp = self.frames[top_addr].clone();
                self.frames[self.pc] = self.frames[swap_addr].clone();
                self.frames[swap_addr] = temp;
            },
            OpType::Dup => {
                let dup_frame_count = op.rm_stack_count as usize;

                let dup_target = self.frames[(self.pc - dup_frame_count) + 1].clone();
                let dup_target = dup_target.inner().inner();
                if let Some(val) = dup_target {
                    self.push(Some(val.as_slice()));
                } else {
                    self.push(None);
                }


            }
            _ => {
                // Other, JumpI, Jump
                let rm_stack_count = op.rm_stack_count as usize;
                let add_stack_count = op.add_stack_count as usize;
                (1..rm_stack_count).into_iter().for_each(|_| {
                    self.pop();
                });
                (1..add_stack_count).into_iter().for_each(|_| {
                    self.push(None);
                });

            },
        }
    }

}