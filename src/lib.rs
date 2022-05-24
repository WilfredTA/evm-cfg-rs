extern crate core;

pub mod op_data;
mod stack;
mod op;
use op::*;
use stack::*;

use std::collections::{HashMap, VecDeque, HashSet};
use std::fmt::Formatter;
use op_data::*;
use bytes::Bytes;
use revm::opcode::*;
use revm::{opcode::*};
use ethers_solc::{project, output, contracts, artifacts};
use petgraph::data::{Element, FromElements};
use primitive_types::{H256, U256};

use petgraph::graph::{DiGraph, Node};
use petgraph::dot::{Dot, Config};
use petgraph::{Directed, Graph};

pub const MAX_STACK_DEPTH: u16 = 1024;

#[derive(Debug, Default)]
pub struct Program {
    pub code: Vec<u8>,
    pub blocks: Vec<Block>,
    pub edges: Vec<(U256, U256)>,
    pub start_addresses: Vec<U256>,
}

pub type CfgNode = Node<CfgNodeData, u64>;
pub type BlockInfo = CfgNodeData;
impl Program {
    pub fn parse_bytecode(mut code: Vec<u8>, entry_sig: Option<[u8; 4]>) -> Self {
        let mut entry_point: U256 = U256::zero();

        let code = code[entry_point.as_usize() .. code.len()].to_vec();

        let mut blocks = vec![];


        let mut ptr: usize = 0;
        let mut prev_ptr = ptr;
        let mut curr_block_codes = vec![];
        let mut entry_points = vec![];
        while ptr < code.len(){
            let curr_byte = code[ptr];
            if let Some(opcode) = OpCode::try_from_u8(curr_byte) {

                let ptr_inc_size = opcode.arg_size() as usize + 1;
                let u8_code = opcode.u8();
                let curr_op = OpCode::try_from_u8(u8_code).unwrap();
                let curr_op_with_metadata = Operation::from(curr_op).pc(ptr.into());
                curr_block_codes.push(curr_op_with_metadata);
                if BLOCK_END_INSTRUCTIONS.contains(&u8_code) ||
                    (ptr + ptr_inc_size) >= code.len() - 1 ||

                    (code[ptr + ptr_inc_size] == JUMPDEST)
                {
                    entry_points.push(U256::from(prev_ptr));

                    let block = Block {
                        pc_start: prev_ptr,
                        pc_end: ptr,
                        ops: curr_block_codes.clone(),
                        successors: vec![],

                    };
                    blocks.push(block);
                    curr_block_codes = vec![];
                    ptr += ptr_inc_size;
                    prev_ptr = ptr;
                } else {
                    ptr += ptr_inc_size;
                }
            } else {
                let slice = &code.as_slice()[prev_ptr..ptr + 1];
                let hexd = hex::encode(&slice);
                println!("In slice {:?}\n Pointer val: {:?}\n Prev Pointer val: {:?}", hexd, ptr, prev_ptr);

                println!("Could not derive OpCode from {:?}", hex::encode(&[curr_byte]));
                println!("Opcodes in this block: {:?}", curr_block_codes);
                entry_points.push(U256::from(prev_ptr));
                let block = Block {
                    pc_start: prev_ptr,
                    pc_end: ptr - 1,
                    ops: curr_block_codes.clone(),
                    successors: vec![],

                };
                blocks.push(block);
                curr_block_codes = vec![];
                let invalid_block = Block {
                    pc_start: ptr,
                    pc_end: ptr,
                    ops: vec![Operation::invalid(curr_byte, Some(ptr.into()))],
                    successors: vec![]
                };
                entry_points.push(U256::from(ptr));
                ptr += 1;
                prev_ptr = ptr;
                blocks.push(invalid_block);

            }
        }
        Program {
            code,
            blocks,
            start_addresses: entry_points,
            edges: vec![]
        }
    }

    pub fn gen_symbolic_edges(&mut self) {
        // Depth first search through CFG.
        // Only consider SWAP, DUP, PUSH, AND, and POP

        // let depth = 1;
        // let mut curr_depth = 0;
        let mut visited: HashSet<(usize, usize)> = HashSet::new();
        let mut queue = VecDeque::new();
        let mut stack = SymbolicStack::new();
        queue.push_front((self.blocks.first().unwrap().clone(), stack.capture()));
         while !queue.is_empty() {
            
            let (mut curr_block, curr_stack) = queue.pop_back().unwrap();
            let mut stack = curr_block.exec_symbolic(SymbolicStack::from(curr_stack), &self.code, curr_block.ops.len() - 1);
            let last_block_op = curr_block.ops.last().unwrap();
            println!("LAST BLOCK OP {:?}", last_block_op);
            if last_block_op.category() == OpType::Jump {
                 let stack_top = stack.peek();
                 println!("STACK TOP: {:?}", stack_top);
                 match stack_top {
                     SymbolicStackValue::Data(data) => {
        //                 // An edge in CFG discovered
                         if let Some(next_block) = self.blocks.iter().find(|blk| {
                             blk.id() == U256::from_big_endian(data.as_slice())
                         }) {
                            println!("Block {} Points to {}", curr_block.id(), next_block.id());
        //                    curr_block.successors.push(next_block.id().as_usize());
                              stack.execute(curr_block.ops.last().unwrap(), &self.code);
                            let edge = (curr_block.id().as_usize(), next_block.id().as_usize());
                            if !visited.contains(&(edge)) {
        //                         visited.insert(edge);
                                 queue.push_front((next_block.clone(), stack.capture()));
        //                         self.edges.push((edge.0.into(), edge.1.into()));
                                
                            }
        //                    curr_depth += 1;
        //                 } else {
        //                     panic!("Invalid jump dest found in symbolic block");
                         }
                     },
                     _ => {}
                 }

            }

        }

        return;

    }


    pub fn gen_concrete_edges(&mut self) {
        let pattern_abs_jumps = vec![OpType::Push, OpType::Jump];
        let pattern_cond_jumps = vec![OpType::Push, OpType::JumpI];
        let abs_jump_edges = self.blocks.iter().filter_map(|block| {
            let outgoing_op_seq = block.get_matching_op_sequences(&pattern_abs_jumps);
            if let Some(push_op_seq) = outgoing_op_seq.first() {
                let push_op = &push_op_seq[0];
                let start_read = push_op.pc.unwrap().as_usize() + 1;
                let end_read = push_op.arg_size as usize + start_read;
                println!("Start read: {}\nEnd read: {}", start_read, end_read);
                let dest = &self.code[start_read..end_read];
                println!("Dest: {:?}", hex::encode(dest));
                let dest = U256::from_big_endian(dest);
                Some((block.id(), dest))
            } else {
                None
            }
        }).collect::<Vec<_>>();

        let mut cond_jump_false_edges = vec![];
        let cond_jump_true_edges = self.blocks.iter().filter_map(|block| {
            let outgoing_op_seq = block.get_matching_op_sequences(&pattern_cond_jumps);
            if let Some(push_op_seq) = outgoing_op_seq.first() {
                let push_op = &push_op_seq[0];
                let start_read = push_op.pc.unwrap().as_usize() + 1;
                let end_read = push_op.arg_size as usize + start_read;
                println!("Start read: {}\nEnd read: {}\n", start_read, end_read);
                let dest = &self.code[start_read..end_read];
                println!("Dest: {:?}", hex::encode(dest));
                let dest = U256::from_big_endian(dest);
                cond_jump_false_edges.push((block.id(), U256::from(block.pc_end + 1)));
                Some((block.id(), dest))


            } else {
                None
            }
        }).collect::<Vec<_>>();

        self.edges.extend(abs_jump_edges.iter());
        self.edges.extend(cond_jump_true_edges.iter());
        self.edges.extend(cond_jump_false_edges.iter())

    }

    pub fn render(&self) -> Graph<BlockInfo, (u64, u64)> {

        let mut id_to_idx = HashMap::new();
        let mut graph_nodes = self.blocks.iter().map(|block| {
            block.to_display_node()
        }).collect::<Vec<_>>();
        graph_nodes.sort_by(|node1, node2| {
            node1.code_loc.cmp(&node2.code_loc)
        });
        let mut i = 0;
        while i < graph_nodes.len() {
            id_to_idx.insert(graph_nodes[i].code_loc, i as u64);
            i += 1;
        }
        let edges = self.edges.iter().map(|edge| {
            let idx_for_id = id_to_idx.get(&edge.0.as_u64()).unwrap();
            let idx_2_for_id = id_to_idx.get(&edge.1.as_u64()).unwrap();
            (idx_for_id.clone() as u32, idx_2_for_id.clone() as u32)
        }).collect::<Vec<_>>();

        let mut g = DiGraph::from_elements(graph_nodes.iter().map(|n| {
            Element::Node { weight: n.clone() }

        }));
        g.extend_with_edges(edges);
        g
    }
}







#[derive(Debug, Clone)]
pub struct Block {
    pub pc_start: usize,
    pub pc_end: usize,
    pub ops: Vec<Operation>,
    pub successors: Vec<usize>
 
}

#[derive(Debug, Clone, Default)]
pub struct CfgNodeData {
    pub ops: String,
    pub code_loc: u64
}

impl Block {
    pub fn id(&self) -> U256 {
        self.pc_start.into()
    }

    pub fn exec_symbolic(&mut self, mut stack: SymbolicStack, code: &[u8], num_codes: usize) -> SymbolicStack {
        (0..num_codes).into_iter().for_each(|code_idx| {
            let op = &self.ops[code_idx];
            stack.execute(op, code);
        });
        
        
        stack
    }

    pub fn to_display_node(&self) -> CfgNodeData {
        let id = self.id();
        let ops = self.ops.iter().map(|op| {
            op.code.as_str().to_string()
        }).collect::<Vec<_>>().join(" ");
        CfgNodeData {
            code_loc: id.as_u64(),
            ops
        }

    }


    pub fn get_matching_op_sequences(&self, optype_sequence: &Vec<OpType>) -> Vec<&[Operation]> {
        let mut i = 0;
        let mut j = optype_sequence.len();
        let mut matches = vec![];
        if self.ops.len() < optype_sequence.len() {
            return vec![];
        }
        while j <= self.ops.len() {
            let sub_slice = &self.ops[i..j];
            let seq = &sub_slice.iter().map(|op| {
                op.category()
            }).collect::<Vec<_>>();
            let is_match = seq.as_slice() == optype_sequence.as_slice();

            if is_match {
                matches.push(sub_slice);
            }
            i += 1;
            j += 1;

        }
        matches
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use ethers_solc::{ProjectPathsConfig, ProjectCompileOutput, MinimalCombinedArtifacts};
    use ethers_solc::project_util::TempProject;
    use hex::encode;

    #[test]
    #[ignore]
    fn compile_counter() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-data/counter");
        let paths = ProjectPathsConfig::builder()
            .sources(root.join("src"));
        let project = TempProject::<MinimalCombinedArtifacts>::new(paths).unwrap();
        let compiled = project.compile().unwrap();
        let ctr = compiled.find("Counter").unwrap().clone();
        println!("{:?}", ctr);
        let contract_raw = ctr.deployed_bytecode.unwrap().bytecode.unwrap().object.into_bytes().unwrap().to_vec();
        println!("{:?}", encode(contract_raw.clone()));
        let mut pgm = Program::parse_bytecode(contract_raw, None);
       // println!("Program: {:#?}", pgm);
        pgm.gen_concrete_edges();
        
        println!("EDGES: {:?}", pgm.edges);
        //
        let edges_from_orphan = pgm.edges.iter().find(|edge| {
            edge.0.as_usize() == 230
        });
        assert!(edges_from_orphan.is_none());
        pgm.gen_symbolic_edges();
        let edges_from_orphan = pgm.edges.iter().find(|edge| {
            edge.0.as_usize() == 230
        });
         assert!(edges_from_orphan.is_some());
        let g = pgm.render();
        println!("{:?}", Dot::with_config(&g, &[Config::EdgeNoLabel]));
      
        assert!(false);
    }

    #[test]
    fn ethereum_pot() {
        let loc = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-data/ethereum_pot");
        let code = std::fs::read_to_string(loc).unwrap();
        let contract_raw = hex::decode(code).unwrap();
        let selector_str = "05e49d1d";
        let selector_bytes = hex::decode(selector_str).unwrap();
        let mut buf = [0u8;4];
        buf.copy_from_slice(selector_bytes.as_slice());
        let mut pgm = Program::parse_bytecode(contract_raw.clone(), None);
        println!("Program: {:#?}", pgm);
        println!("Block count: {}", pgm.blocks.len());
        println!("Entry points count: {}", pgm.start_addresses.len());
        pgm.gen_symbolic_edges();
        let final_block_start = pgm.start_addresses.last().unwrap().clone().as_usize();
        let op = contract_raw[final_block_start];
        let g = pgm.render();
        println!("Final entry: {:?}\nopcode: {:?}", final_block_start, op);
        println!("{:?}", Dot::with_config(&g, &[Config::EdgeNoLabel]));

        assert!(false);
    }

}
