use core::panic;
use std::collections::HashMap;

use super::{
    asm::asm::AssemblyInstruction,
    vm::instructions::{
        get_all_instruction_layouts, BitLayout, Instruction, InstructionTable,
        LoadStoreAddressingMode, PartType,
    },
};

pub fn truncate_to_bits(num: u32, bits: u32) -> u32 {
    (num << (32 - bits)) >> (32 - bits)
}

pub fn delete_msb_bits(num: u32, bits: u32) -> u32 {
    (num << bits) >> bits
}

pub fn encode_stackoffset(offset: u32) -> u32 {
    let bit_pattern: u32 = 0b01101 << 27;
    let bytes: u32 = truncate_to_bits(offset, 27);

    return bit_pattern + bytes;
}

pub fn encode_instruction(ins: &AssemblyInstruction) -> u32 {
    match ins {
        AssemblyInstruction::StackOffset { bytes } => {
            return encode_stackoffset(*bytes);
        }
        _ => 0,
    }
}

pub fn encode_asm(code: &[AssemblyInstruction]) -> Vec<u32> {
    code.iter().map(|x| encode_instruction(x)).collect()
}

pub struct InstructionEncoder<'a> {
    pub layout: &'a BitLayout,
    pub current: u32,
}

impl<'a> InstructionEncoder<'a> {
    
    pub fn encode_bytes(&mut self, part: &str, value: &[u8]) -> &mut Self {
        let as_u32 = {
            if value.len() < 4 {
                let mut vec = vec![];
                vec.extend(value);

                for _ in value.len() .. 4 {
                    vec.push(0);
                }
                
                u32::from_le_bytes(vec.try_into().unwrap())
            }else {
                u32::from_le_bytes(value.try_into().unwrap())
            }
        };
        self.encode(part, as_u32)
    }

    pub fn encode(&mut self, part: &str, value: u32) -> &mut Self {
        let mut bit_offset = 5;
        let mut found = false;
        for layout_part in self.layout.layout.iter() {
            if layout_part.name == part {
                found = true;
                match &layout_part.layout_type {
                    PartType::BitPattern(patterns) => {
                        let pattern = patterns.iter().find(|x| x.value == value).unwrap();
                        let offseted = delete_msb_bits(pattern.pattern, bit_offset);
                        let position_offset = (32 - bit_offset) - layout_part.length as u32;
                        let positioned = offseted << position_offset;
                        self.current += positioned;
                        break;
                    }
                    PartType::Immediate => {
                        let offseted = delete_msb_bits(value, bit_offset);
                        let position_offset = (32 - bit_offset) - layout_part.length as u32;
                        let positioned = offseted << position_offset;
                        self.current += positioned;
                        break;
                    }
                }
            }
            bit_offset += layout_part.length as u32;
        }
        if !found {
            panic!("Could not find instruction part {part}");
        }
        self
    }

    pub fn make(&self) -> u32 {
        self.current
    }
}

pub struct InstructionDecoder<'a> {
    pub layout: &'a BitLayout,
    pub instruction: u32,
}

impl<'a> InstructionDecoder<'a> {
    pub fn decode(&self) -> Instruction {
        let pseudoop = self.layout.instruction_pseudoop;

        match pseudoop {
            0 => Instruction::Noop,
            0b00001 => {
                let (num_bytes_pattern, _) = self.layout.get_part("num bytes", self.instruction);
                let (shift_pattern, shift_value) = self.layout.get_part("lshift", self.instruction);
                let immediate_lsb = self.layout.get_part("immediate lsb", self.instruction);
                return Instruction::PushImmediate {
                    bytes: (num_bytes_pattern as u8).into(),
                    immediate: (immediate_lsb.0 as u16).to_le_bytes(),
                    lshift: (shift_pattern as u8).into(),
                };
            }
            0b01101 => {
                let (_, value) = self.layout.get_part("num bytes", self.instruction);
                return Instruction::StackOffset { bytes: value };
            }
            0b00010 => {
                let (bytes_pattern, _) = self.layout.get_part("num bytes", self.instruction);
                let (mode_pattern, _) = self.layout.get_part("mode", self.instruction);
                let (_, operand_value) = self.layout.get_part("operand", self.instruction);
                return Instruction::LoadAddress {
                    bytes: (bytes_pattern as u8).into(),
                    mode: (mode_pattern as u8).into(),
                    operand: operand_value,
                };
            }
            0b00011 => {
                let (bytes_pattern, _) = self.layout.get_part("num bytes", self.instruction);
                let (mode_pattern, _) = self.layout.get_part("mode", self.instruction);
                let (_, operand_value) = self.layout.get_part("operand", self.instruction);
                return Instruction::StoreAddress {
                    bytes: (bytes_pattern as u8).into(),
                    mode: (mode_pattern as u8).into(),
                    operand: operand_value,
                };
            }
            0b00100 => {
                let (bytes_pattern, _) = self.layout.get_part("num bytes", self.instruction);
                let (direction_pattern, _) = self.layout.get_part("direction", self.instruction);
                let (mode_pattern, _) = self.layout.get_part("mode", self.instruction);
                let (sign_pattern, _) = self.layout.get_part("keep sign", self.instruction);
                let (_, operand_value) = self.layout.get_part("operand", self.instruction);
                return Instruction::BitShift {
                    bytes: (bytes_pattern as u8).into(),
                    mode: (mode_pattern as u8).into(),
                    direction: (direction_pattern as u8).into(),
                    sign: (sign_pattern as u8).into(),
                    operand: operand_value as u8,
                };
            }
            0b00101 => {
                let (bytes_pattern, _) = self.layout.get_part("num bytes", self.instruction);
                let (operation_pattern, _) = self.layout.get_part("operation", self.instruction);
                let (sign_pattern, _) = self.layout.get_part("sign", self.instruction);
                let (mode_pattern, _) = self.layout.get_part("mode", self.instruction);
                let (_, operand_value) = self.layout.get_part("operand", self.instruction);
                return Instruction::Bitwise {
                    bytes: (bytes_pattern as u8).into(),
                    mode: (mode_pattern as u8).into(),
                    operation: (operation_pattern as u8).into(),
                    sign: (sign_pattern as u8).into(),
                    operand: (operand_value as u16).to_le_bytes(),
                };
            }
            0b00110 => {
                let (bytes_pattern, _) = self.layout.get_part("num bytes", self.instruction);
                let (operation_pattern, _) = self.layout.get_part("operation", self.instruction);
                let (sign_pattern, _) = self.layout.get_part("sign", self.instruction);
                let (mode_pattern, _) = self.layout.get_part("mode", self.instruction);
                let (_, operand_value) = self.layout.get_part("operand", self.instruction);
                return Instruction::IntegerArithmetic {
                    bytes: (bytes_pattern as u8).into(),
                    sign: (sign_pattern as u8).into(),
                    mode: (mode_pattern as u8).into(),
                    operation: (operation_pattern as u8).into(),
                    operand: (operand_value as u16).to_le_bytes(),
                };
            }
            0b00111 => {
                let (bytes_pattern, _) = self.layout.get_part("num bytes", self.instruction);
                let (operation_pattern, _) = self.layout.get_part("operation", self.instruction);
                let (sign_pattern, _) = self.layout.get_part("sign", self.instruction);
                let (mode_pattern, _) = self.layout.get_part("mode", self.instruction);
                let (_, operand_value) = self.layout.get_part("operand", self.instruction);
                return Instruction::IntegerCompare {
                    bytes: (bytes_pattern as u8).into(),
                    sign: (sign_pattern as u8).into(),
                    mode: (mode_pattern as u8).into(),
                    operation: (operation_pattern as u8).into(),
                    operand: (operand_value as u16).to_le_bytes(),
                };
            }
            0b01000 => {
                let (bytes_pattern, _) = self.layout.get_part("num bytes", self.instruction);
                let (operation_pattern, _) = self.layout.get_part("operation", self.instruction);
                return Instruction::FloatArithmetic {
                    bytes: (bytes_pattern as u8).into(),
                    operation: (operation_pattern as u8).into(),
                };
            }
            0b01001 => {
                let (bytes_pattern, _) = self.layout.get_part("num bytes", self.instruction);
                let (operation_pattern, _) = self.layout.get_part("operation", self.instruction);
                return Instruction::FloatCompare {
                    bytes: (bytes_pattern as u8).into(),
                    operation: (operation_pattern as u8).into(),
                };
            }
            0b01010 => {
                let (register_pattern, _) = self.layout.get_part("register", self.instruction);
                return Instruction::PushFromRegister {
                    control_register: (register_pattern as u8).into(),
                };
            }
            0b01011 => {
                let (register_pattern, _) = self.layout.get_part("register", self.instruction);
                return Instruction::PopIntoRegister {
                    control_register: (register_pattern as u8).into(),
                };
            }
            0b01100 => {
                let (bytes_pattern, _) = self.layout.get_part("num bytes", self.instruction);
                return Instruction::Pop {
                    bytes: (bytes_pattern as u8).into(),
                };
            }
            0b01110 => {
                let (source_pattern, _) = self.layout.get_part("source", self.instruction);
                let (_, offset) = self.layout.get_part("offset", self.instruction);
                return Instruction::Call {
                    source: (source_pattern as u8).into(),
                    offset,
                };
            }
            0b01111 => {
                return Instruction::Return;
            }
            _ => {
                panic!("Not recognized: {inst:#05b}", inst = pseudoop as u8)
            }
        };

        Instruction::Noop
    }
}

pub struct LayoutHelper {
    pub table: InstructionTable,
}

impl LayoutHelper {
    pub fn new() -> LayoutHelper {
        let table = get_all_instruction_layouts();

        return LayoutHelper { table };
    }

    pub fn begin_encode(&self, name: &str) -> InstructionEncoder {
        let instruction = self.table.table.get(name).unwrap();

        InstructionEncoder {
            layout: instruction,
            current: (instruction.instruction_pseudoop as u32) << 27,
        }
    }

    pub fn encode_instruction(&self, instruction: &Instruction) -> u32 {
        match instruction {
            Instruction::Noop => 0,
            Instruction::StackOffset { bytes } => self
                .begin_encode("stackoffset")
                .encode("num bytes", *bytes)
                .make(),
            Instruction::PushImmediate {
                bytes,
                lshift,
                immediate,
            } => self
                .begin_encode("push_imm")
                .encode("num bytes", bytes.get_bytes() as u32)
                .encode("lshift", lshift.get_shift_size() as u32)
                .encode_bytes("immediate lsb",immediate)
                .make(),
            Instruction::LoadAddress {
                bytes,
                mode,
                operand,
            } => self
                .begin_encode("loadaddr")
                .encode("num bytes", bytes.get_bytes() as u32)
                .encode("mode", mode.get_bit_pattern() as u32)
                .encode("operand", *operand)
                .make(),
            Instruction::StoreAddress {
                bytes,
                mode,
                operand,
            } => self
                .begin_encode("storeaddr")
                .encode("num bytes", bytes.get_bytes() as u32)
                .encode("mode", mode.get_bit_pattern() as u32)
                .encode("operand", *operand as u32)
                .make(),
            Instruction::BitShift {
                bytes,
                direction,
                mode,
                sign,
                operand,
            } => self
                .begin_encode("shift")
                .encode("num bytes", bytes.get_bytes() as u32)
                .encode("direction", direction.get_bit_pattern() as u32)
                .encode("mode", mode.get_bit_pattern() as u32)
                .encode("keep sign", sign.get_bit_pattern() as u32)
                .encode("operand", *operand as u32)
                .make(),
            Instruction::Bitwise {
                bytes,
                operation,
                sign,
                mode,
                operand,
            } => self
                .begin_encode("bitwise")
                .encode("num bytes", bytes.get_bytes() as u32)
                .encode("operation", operation.get_bit_pattern() as u32)
                .encode("mode", mode.get_bit_pattern() as u32)
                .encode("sign", sign.get_bit_pattern() as u32)
                .encode_bytes("operand", operand)
                .make(),
            Instruction::IntegerArithmetic {
                bytes,
                operation,
                sign,
                mode,
                operand,
            } => self
                .begin_encode("integer_binary_op")
                .encode("num bytes", bytes.get_bytes() as u32)
                .encode("operation", operation.get_bit_pattern() as u32)
                .encode("sign", sign.get_bit_pattern() as u32)
                .encode("mode", mode.get_bit_pattern() as u32)
                .encode_bytes("operand", operand)
                .make(),
            Instruction::IntegerCompare {
                bytes,
                operation,
                sign,
                mode,
                operand,
            } => self
                .begin_encode("integer_compare")
                .encode("num bytes", bytes.get_bytes() as u32)
                .encode("operation", operation.get_bit_pattern() as u32)
                .encode("sign", sign.get_bit_pattern() as u32)
                .encode("mode", mode.get_bit_pattern() as u32)
                .encode_bytes("operand", operand)
                .make(),
            Instruction::FloatArithmetic { bytes, operation } => self
                .begin_encode("float_binary_op")
                .encode("num bytes", bytes.get_bytes() as u32)
                .encode("operation", operation.get_bit_pattern() as u32)
                .make(),
            Instruction::FloatCompare { bytes, operation } => self
                .begin_encode("float_compare_op")
                .encode("num bytes", bytes.get_bytes() as u32)
                .encode("operation", operation.get_bit_pattern() as u32)
                .make(),
            Instruction::PushFromRegister { control_register } => self
                .begin_encode("push_reg")
                .encode("register", control_register.get_bit_pattern() as u32)
                .make(),
            Instruction::PopIntoRegister { control_register } => self
                .begin_encode("pop_reg")
                .encode("register", control_register.get_bit_pattern() as u32)
                .make(),
            Instruction::Pop { bytes } => self
                .begin_encode("pop")
                .encode("num bytes", bytes.get_bytes() as u32)
                .make(),
            Instruction::Call { source, offset } => self
                .begin_encode("call")
                .encode("source", source.get_bit_pattern() as u32)
                .encode("offset", *offset)
                .make(),
            Instruction::JumpIfZero { source, offset } => self
                .begin_encode("jz")
                .encode("source", source.get_bit_pattern() as u32)
                .encode("offset", *offset)
                .make(),
            Instruction::JumpIfNotZero { source, offset } => self
                .begin_encode("jnz")
                .encode("source", source.get_bit_pattern() as u32)
                .encode("offset", *offset)
                .make(),
            Instruction::JumpUnconditional { source, offset } => self
                .begin_encode("jmp")
                .encode("source", source.get_bit_pattern() as u32)
                .encode("offset", *offset)
                .make(),
            Instruction::Exit => self.begin_encode("exit").make(),
            Instruction::Return => self.begin_encode("return").make(),
        }
    }

    pub fn begin_decode(&self, instruction: u32) -> InstructionDecoder {
        let pseudo_op = (instruction >> 27) as u8;
        let instruction_name = self.table.pseudoops.get(&pseudo_op);
        match instruction_name {
            Some(name) => {
                let layout = self.table.table.get(name).unwrap();
                InstructionDecoder {
                    layout: layout,
                    instruction,
                }
            }
            None => {
                panic!("No instruction found for pseudo op {pseudo_op:#05b}")
            }
        }
    }
}

#[cfg(test)]
mod tests {


    #[cfg(test)]
    use pretty_assertions::assert_eq;

    use crate::freyr::{encoder::*, vm::instructions::*};

    #[test]
    fn encode_decode_push_immediate32_lshift16() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("push_imm")
            .encode("num bytes", 4)
            .encode("lshift", 16)
            .encode_bytes("immediate lsb", &25u16.to_le_bytes())
            .make();

        let decoded = encoder.begin_decode(encoded).decode();
        assert_eq!(
            decoded,
            Instruction::PushImmediate {
                bytes: NumberOfBytes::Bytes4,
                lshift: LeftShift::Shift16,
                immediate: 25u16.to_le_bytes()
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_push_immediate16_nolshift() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("push_imm")
            .encode("num bytes", 2)
            .encode("lshift", 0)
            .encode_bytes("immediate lsb", &250u16.to_le_bytes())
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::PushImmediate {
                bytes: NumberOfBytes::Bytes2,
                lshift: LeftShift::None,
                immediate: 250u16.to_le_bytes()
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_loadaddr_stack_32bits() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("loadaddr")
            .encode("num bytes", 2)
            .encode("mode", 0)
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::LoadAddress {
                bytes: NumberOfBytes::Bytes2,
                mode: LoadStoreAddressingMode::Stack,
                operand: 0
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_loadaddr_relative_pos_32bits() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("loadaddr")
            .encode("num bytes", 2)
            .encode("mode", 1)
            .encode("operand", 45)
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::LoadAddress {
                bytes: NumberOfBytes::Bytes2,
                mode: LoadStoreAddressingMode::RelativeForward,
                operand: 45
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_loadaddr_relative_neg_64bits() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("loadaddr")
            .encode("num bytes", 8)
            .encode("mode", 2)
            .encode("operand", 453)
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::LoadAddress {
                bytes: NumberOfBytes::Bytes8,
                mode: LoadStoreAddressingMode::RelativeBackward,
                operand: 453
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_loadaddr_absolute_8bits() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("loadaddr")
            .encode("num bytes", 1)
            .encode("mode", 3)
            .encode("operand", 123)
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::LoadAddress {
                bytes: NumberOfBytes::Bytes1,
                mode: LoadStoreAddressingMode::Absolute,
                operand: 123
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_storeaddr_stack_32bits() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("storeaddr")
            .encode("num bytes", 2)
            .encode("mode", 0)
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::StoreAddress {
                bytes: NumberOfBytes::Bytes2,
                mode: LoadStoreAddressingMode::Stack,
                operand: 0
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_storeaddr_relative_pos_32bits() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("storeaddr")
            .encode("num bytes", 2)
            .encode("mode", 1)
            .encode("operand", 45)
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::StoreAddress {
                bytes: NumberOfBytes::Bytes2,
                mode: LoadStoreAddressingMode::RelativeForward,
                operand: 45
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_storeaddr_relative_neg_64bits() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("storeaddr")
            .encode("num bytes", 8)
            .encode("mode", 2)
            .encode("operand", 453)
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::StoreAddress {
                bytes: NumberOfBytes::Bytes8,
                mode: LoadStoreAddressingMode::RelativeBackward,
                operand: 453
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_storeaddr_absolute_8bits() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("storeaddr")
            .encode("num bytes", 1)
            .encode("mode", 3)
            .encode("operand", 123)
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::StoreAddress {
                bytes: NumberOfBytes::Bytes1,
                mode: LoadStoreAddressingMode::Absolute,
                operand: 123
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_shift_left_stack() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("shift")
            .encode("num bytes", 4)
            .encode("direction", 0)
            .encode("mode", 0)
            .encode("keep sign", 0)
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::BitShift {
                bytes: NumberOfBytes::Bytes4,
                direction: ShiftDirection::Left,
                mode: OperationMode::PureStack,
                sign: SignFlag::Unsigned,
                operand: 0
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_shift_right_immediate() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("shift")
            .encode("num bytes", 8)
            .encode("direction", 1)
            .encode("mode", 1)
            .encode("operand", 12)
            .encode("keep sign", 1)
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::BitShift {
                bytes: NumberOfBytes::Bytes8,
                direction: ShiftDirection::Right,
                mode: OperationMode::StackAndImmediate,
                sign: SignFlag::Signed,
                operand: 12
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_bitwise_operation_and_stack() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("bitwise")
            .encode("num bytes", 4)
            .encode("operation", 0b00)
            .encode("mode", 0)
            .encode("sign", 0)
            .encode_bytes("operand", &[0, 0])
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::Bitwise {
                bytes: NumberOfBytes::Bytes4,
                operation: BitwiseOperation::And,
                mode: OperationMode::PureStack,
                sign: SignFlag::Unsigned,
                operand: [0, 0]
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_bitwise_operation_and_stack_signed() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("bitwise")
            .encode("num bytes", 4)
            .encode("operation", 0b00)
            .encode("mode", 0)
            .encode("sign", 1)
            .encode_bytes("operand", &[0, 0])
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::Bitwise {
                bytes: NumberOfBytes::Bytes4,
                operation: BitwiseOperation::And,
                mode: OperationMode::PureStack,
                sign: SignFlag::Signed,
                operand: [0, 0]
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_bitwise_operation_or_immediate_unsigned() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("bitwise")
            .encode("num bytes", 4)
            .encode("operation", 0b01)
            .encode("sign", 0)
            .encode("mode", 1)
            .encode_bytes("operand", &123u16.to_le_bytes())
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::Bitwise {
                bytes: NumberOfBytes::Bytes4,
                operation: BitwiseOperation::Or,
                mode: OperationMode::StackAndImmediate,
                sign: SignFlag::Unsigned,
                operand: 123u16.to_le_bytes()
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_bitwise_64_operation_xor_immediate() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("bitwise")
            .encode("num bytes", 8)
            .encode("operation", 0b10)
            .encode("sign", 0)
            .encode("mode", 1)
            .encode_bytes("operand", &65535u16.to_le_bytes())
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::Bitwise {
                bytes: NumberOfBytes::Bytes8,
                operation: BitwiseOperation::Xor,
                mode: OperationMode::StackAndImmediate,
                sign: SignFlag::Unsigned,
                operand: 65535u16.to_le_bytes()
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_arithmetic_operation_add_32bits_signed_stack() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("integer_binary_op")
            .encode("num bytes", 4)
            .encode("operation", 0b000)
            .encode("sign", 1)
            .encode("mode", 0)
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::IntegerArithmetic {
                bytes: NumberOfBytes::Bytes4,
                sign: SignFlag::Signed,
                operation: ArithmeticOperation::Sum,
                mode: OperationMode::PureStack,
                operand: 0u16.to_le_bytes()
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_arithmetic_operation_mul_64bits_unsigned_imm() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("integer_binary_op")
            .encode("num bytes", 8)
            .encode("operation", 0b010)
            .encode("sign", 0)
            .encode("mode", 1)
            .encode("operand", 65535)
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::IntegerArithmetic {
                bytes: NumberOfBytes::Bytes8,
                sign: SignFlag::Unsigned,
                operation: ArithmeticOperation::Multiply,
                mode: OperationMode::StackAndImmediate,
                operand: 65535u16.to_le_bytes()
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_arithmetic_operation_pow_8bits_unsigned_imm() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("integer_binary_op")
            .encode("num bytes", 1)
            .encode("operation", 0b100)
            .encode("sign", 0)
            .encode("mode", 1)
            .encode("operand", 15)
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::IntegerArithmetic {
                bytes: NumberOfBytes::Bytes1,
                sign: SignFlag::Unsigned,
                operation: ArithmeticOperation::Power,
                mode: OperationMode::StackAndImmediate,
                operand: 15i16.to_le_bytes()
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_compare_operation_eq_32bits_unsigned_imm() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("integer_compare")
            .encode("num bytes", 4)
            .encode("operation", 0b000)
            .encode("sign", 0)
            .encode("mode", 1)
            .encode_bytes("operand", &15u16.to_le_bytes())
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::IntegerCompare {
                bytes: NumberOfBytes::Bytes4,
                sign: SignFlag::Unsigned,
                operation: CompareOperation::Equals,
                mode: OperationMode::StackAndImmediate,
                operand: 15u16.to_le_bytes()
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_compare_operation_gte_16bits_signed_stack() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("integer_compare")
            .encode("num bytes", 2)
            .encode("operation", 0b101)
            .encode("sign", 1)
            .encode("mode", 0)
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::IntegerCompare {
                bytes: NumberOfBytes::Bytes2,
                sign: SignFlag::Signed,
                operation: CompareOperation::GreaterThanOrEquals,
                mode: OperationMode::PureStack,
                operand: [0,0]
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_arithmetic_operation_add_float_32bits() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("float_binary_op")
            .encode("num bytes", 4)
            .encode("operation", 0b000)
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::FloatArithmetic {
                bytes: NumberOfBytes::Bytes4,
                operation: ArithmeticOperation::Sum
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_arithmetic_operation_div_float_64bits() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("float_binary_op")
            .encode("num bytes", 8)
            .encode("operation", 0b011)
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::FloatArithmetic {
                bytes: NumberOfBytes::Bytes8,
                operation: ArithmeticOperation::Divide
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_compare_operation_eq_float_32bits() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("float_compare_op")
            .encode("num bytes", 4)
            .encode("operation", 0b000)
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::FloatCompare {
                bytes: NumberOfBytes::Bytes4,
                operation: CompareOperation::Equals
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_compare_operation_gte_float_64bits() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("float_compare_op")
            .encode("num bytes", 8)
            .encode("operation", 0b101)
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::FloatCompare {
                bytes: NumberOfBytes::Bytes8,
                operation: CompareOperation::GreaterThanOrEquals
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_push_reg_bp() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("push_reg")
            .encode("register", 0b00)
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::PushFromRegister {
                control_register: ControlRegister::BasePointer
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_pop_reg_ip() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("pop_reg")
            .encode("register", 0b10)
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::PopIntoRegister {
                control_register: ControlRegister::InstructionPointer
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_pop_stack() {
        let encoder = LayoutHelper::new();
        let encoded = encoder.begin_encode("pop").encode("num bytes", 8).make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::Pop {
                bytes: NumberOfBytes::Bytes8
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_stackoffset() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("stackoffset")
            .encode("num bytes", 12347)
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(decoded, Instruction::StackOffset { bytes: 12347 });

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_call_from_operand() {
        let encoder = LayoutHelper::new();
        let encoded = encoder
            .begin_encode("call")
            .encode("source", 0)
            .encode("offset", 151)
            .make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::Call {
                source: AddressJumpAddressSource::FromOperand,
                offset: 151
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_call_from_stack() {
        let encoder = LayoutHelper::new();
        let encoded = encoder.begin_encode("call").encode("source", 1).make();

        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(
            decoded,
            Instruction::Call {
                source: AddressJumpAddressSource::PopFromStack,
                offset: 0
            }
        );

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }

    #[test]
    fn encode_decode_return() {
        let encoder = LayoutHelper::new();
        let encoded = encoder.begin_encode("return").make();
        let decoded = encoder.begin_decode(encoded).decode();

        assert_eq!(decoded, Instruction::Return);

        let reencoded = encoder.encode_instruction(&decoded);
        assert_eq!(reencoded, encoded);

        let redecoded = encoder.begin_decode(reencoded).decode();
        assert_eq!(redecoded, decoded);
    }
}
