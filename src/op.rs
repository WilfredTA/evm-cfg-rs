use revm::OpCode;
use primitive_types::U256;
use revm::opcode::*;
use crate::op_data::*;

#[derive(Clone, Debug, Default)]
pub struct Operation {
    pub code: OpCode,
    pub rm_stack_count: u8,
    pub add_stack_count: u8,
    pub arg_size: u8,
    pub is_invalid: bool,
    pub pc: Option<U256>,
}

#[derive(Eq, PartialEq, Debug)]
pub enum OpType {
    Jump,
    JumpI,
    Dup,
    Push,
    Swap,
    And,
    Pop,
    Other

}
impl Operation {
    pub fn invalid(invalid_code: u8, pc: Option<U256>) -> Self {
        Operation {
            code: OpCode::invalid(invalid_code),
            rm_stack_count: 0,
            add_stack_count: 0,
            arg_size: 0,
            is_invalid: true,
            pc,
        }
    }

    pub fn pc(mut self, addr: U256) -> Self {
        self.pc = Some(addr);
        self
    }

    pub fn category(&self) -> OpType {
        let u8_code = self.code.u8();
        if let Some(push_amt) = OpCode::is_push(u8_code) {
            OpType::Push
        } else if let Some(swap_depth) = OpCode::is_swap(u8_code) {
            OpType::Swap
        } else if let Some(dup_depth) = OpCode::is_dup(u8_code) {
            OpType::Dup
        } else if u8_code == AND {
            OpType::And
        } else if u8_code == POP {
            OpType::Pop
        } else if u8_code == JUMP {
            OpType::Jump
        } else if u8_code == JUMPI {
            OpType::JumpI
        } else {
            OpType::Other
        }
    }

}


impl From<OpCode> for Operation {
    fn from(code: OpCode) -> Self {
        let arg_size = code.arg_size();
        let u8_code = code.u8();

        let stack_add_size = {
            if NON_STACK_INCREASING_OPS.contains(&u8_code) {
                0_u8
            } else {
                1
            }
        };

        let stack_rm_size = {
            if (0x80..=0x9f_u8).contains(&u8_code) {
                0_u8
            } else if let Some(depth) = OPCODE_STACK_ARG_DEPTH[u8_code as usize] {
                depth
            } else {
                0
            }
        };



        Operation {
            code: OpCode::try_from_u8(u8_code).unwrap(),
            rm_stack_count: stack_rm_size,
            add_stack_count: stack_add_size,
            arg_size,
            is_invalid: false,
            pc: None,
        }
    }
}