use std::ops::RangeBounds;

use primitive_types::U256;
use num_bigint::BigUint;

struct Gas {
    current: u64
}

impl Gas {
    fn decrement(&mut self, n: &u64) {
        self.current -= n;
    }

    fn increment(&mut self, n: &u64) {
        self.current += n;
    }
}

pub struct EvmResult {
    pub stack: Vec<U256>,
    pub success: bool,
}

pub fn evm(_code: impl AsRef<[u8]>) -> EvmResult {
    let mut stack: Vec<U256> = Vec::new();
    let mut pc = 0;
    let mut stop_flag = false;
    let _gas: Gas = Gas {
        current: 100000000
    };

    let code = _code.as_ref();

    while pc < code.len() && !stop_flag {
        let opcode = code[pc];
        pc += 1;

        // STOP
        if opcode == 0x00 {
            stop_flag = true;
        }

        // PUSH0..PUSH32
        if (0x5F..=0x7F).contains(&opcode) {
            let byte_amount = (opcode - 0x5F) as usize;
            if byte_amount > 0 {
                let mut value = U256::zero();
                // Read the next byte_amount bytes and build the value
                for i in 0..byte_amount {
                    if pc + i < code.len() {
                        value <<= 8;  // Shift left by 8 bits
                        value = value | U256::from(code[pc + i]); // insert byte into byte slot
                    }
                }
                stack.insert(0, value);
                pc += byte_amount;
            } else {
                // PUSH0 case
                stack.insert(0, U256::zero());
            }
        }

        // POP
        if opcode == 0x50 {
            stack.remove(0);
        }

        // ADD
        if opcode == 0x01 {
            let a = stack.remove(1);
            let b = stack.remove(0);
            let result = a.overflowing_add(b).0;
            stack.insert(0, result);
        }

        // MUL 
        if opcode == 0x02 {
            let a = stack.remove(1);
            let b = stack.remove(0);
            let result = a.overflowing_mul(b).0;
            stack.insert(0, result);
        }

        // SUB
        if opcode == 0x03 {
            let a = stack.remove(1);
            let b = stack.remove(0);
            let result = b.overflowing_sub(a).0;
            stack.insert(0, result);
        }

        // DIV 
        if opcode == 0x04 {
            let a = stack.remove(1); // denominator
            let b = stack.remove(0); // numerator
            if a == U256::zero() { 
                stack.insert(0, U256::zero());
            } else {
                let result = b / a;
                stack.insert(0, result);
            }
        }

        // MOD 
        if opcode == 0x06 {
            let a = stack.remove(1); // denominator
            let b = stack.remove(0); // numerator
            if a == U256::zero() { 
                stack.insert(0, U256::zero());
            } else {
                let result = b % a;
                stack.insert(0, result);
            }
        }

        // ADDMOD
        if opcode == 0x08 {
            let n = stack.remove(2);
            let a = stack.remove(1);
            let b = stack.remove(0);
            if n == U256::zero() { 
                stack.insert(0, U256::zero());
            } else {
                let result = (a.overflowing_add(b).0) % n;
                stack.insert(0, result);
            }
        }

        // MULMOD
        if opcode == 0x09 {
            let n = stack.remove(2);
            let a = stack.remove(1);
            let b = stack.remove(0);
            if n == U256::zero() { 
                stack.insert(0, U256::zero());
            } else {
                // NOTE this logic differs from ADDMOD because a.overflowing_mul(b) wasn't evaluating correctly
                // so I imported the num_bigint library
                let mut a_bytes = [0u8; 32];
                let mut b_bytes = [0u8; 32];
                let mut n_bytes = [0u8; 32];

                a.to_big_endian(&mut a_bytes);
                b.to_big_endian(&mut b_bytes);
                n.to_big_endian(&mut n_bytes);

                let a_big = BigUint::from_bytes_be(&a_bytes);
                let b_big = BigUint::from_bytes_be(&b_bytes);
                let n_big = BigUint::from_bytes_be(&n_bytes);

                // Perform multiplication and modulo with full precision
                let result_big = (a_big * b_big) % n_big;

                // Convert back to U256
                let result_bytes = result_big.to_bytes_be();
                let mut result_array = [0u8; 32];
                if result_bytes.len() <= 32 {
                    result_array[32 - result_bytes.len()..].copy_from_slice(&result_bytes);
                }
                stack.insert(0, U256::from_big_endian(&result_array));
            }
        }

        // EXP
        if opcode == 0x0A {
            let a = stack.remove(1);
            let b = stack.remove(0);
            let result = b.overflowing_pow(a).0;
            stack.insert(0, result);
        }

        // SIGEXTEND
        if opcode == 0x0B {
            let byte_pos = stack.remove(0);
            let value = stack.remove(0);
            
            // If byte_pos >= 32, just push the value back unchanged
            if byte_pos >= U256::from(32) {
                stack.insert(0, value);
            } else {
                let byte_pos = byte_pos.as_u64() as usize;
                let mut bytes = [0u8; 32];
                value.to_big_endian(&mut bytes);
                
                // Get the sign bit from the specified byte position
                let sign_bit = (bytes[31 - byte_pos] & 0x80) != 0;
                
                // Fill all higher bytes with 1s if sign bit is 1, or 0s if sign bit is 0
                bytes.iter_mut().take(31 - byte_pos).for_each(|b| *b = if sign_bit { 0xFF } else { 0x00 });
                
                stack.insert(0, U256::from_big_endian(&bytes));
            }
        }

        // SDIV
        if opcode == 0x05 {
            let denominator = stack.remove(1); 
            let numerator = stack.remove(0);
            
            if denominator == U256::zero() {
                stack.insert(0, U256::zero());
            } else {
                // Check if numerator is -2^255 (minimum value)
                let min_value = U256::from(1) << 255;
                let is_numerator_min = numerator == min_value;
                
                // Get signs
                let numerator_negative = (numerator >> 255) == U256::from(1);
                let denominator_negative = (denominator >> 255) == U256::from(1);
                
                // Convert to absolute values
                let abs_numerator = if numerator_negative { (!numerator).overflowing_add(U256::from(1)).0 } else { numerator };
                let abs_denominator = if denominator_negative { (!denominator).overflowing_add(U256::from(1)).0 } else { denominator };
                
                // Perform division
                let mut result = abs_numerator / abs_denominator;
                
                // Handle special case: -2^255 / -1
                if is_numerator_min && denominator == U256::MAX {
                    result = min_value;
                } else if numerator_negative != denominator_negative {
                    // Result should be negative
                    result = (!result).overflowing_add(U256::from(1)).0;
                }
                
                stack.insert(0, result);
            }
        }

        // SMOD
        if opcode == 0x07 {
            let denominator = stack.remove(1); 
            let numerator = stack.remove(0);

            if denominator == U256::zero() {
                stack.insert(0, U256::zero());
            } else {
                // Get signs by checking most significant bit
                let numerator_negative = (numerator >> 255) == U256::from(1);
                let denominator_negative = (denominator >> 255) == U256::from(1);

                // Convert to absolute values
                let abs_numerator = if numerator_negative { (!numerator).overflowing_add(U256::from(1)).0 } else { numerator };
                let abs_denominator = if denominator_negative { (!denominator).overflowing_add(U256::from(1)).0 } else { denominator };

                // Perform modulo on absolute values
                let mut result = abs_numerator % abs_denominator;

                // If numerator was negative, result should be negative
                if numerator_negative && result != U256::zero() {
                    result = (!result).overflowing_add(U256::from(1)).0;
                }

                stack.insert(0, result);
            }
        }

        // LT
        if opcode == 0x10 {
            let right_side = stack.remove(1);
            let left_side = stack.remove(0);
            stack.insert(0, if left_side < right_side { U256::one() } else { U256::zero() });
        }

        // GT
        if opcode == 0x11 {
            let right_side = stack.remove(1);
            let left_side = stack.remove(0);
            stack.insert(0, if left_side > right_side { U256::one() } else { U256::zero() });
        }

        // SLT 
        if opcode == 0x12 {
            let right_side = stack.remove(1);
            let left_side = stack.remove(0);
            
            let left_negative = (left_side >> 255) == U256::from(1);
            let right_negative = (right_side >> 255) == U256::from(1);

            if left_negative == right_negative { 
                // handle same sign with absolutes
                let abs_left = if left_negative { (!left_side).overflowing_add(U256::from(1)).0 } else { left_side };
                let abs_right = if right_negative { (!right_side).overflowing_add(U256::from(1)).0 } else { right_side };

                let result = if left_negative { abs_right < abs_left } else { abs_left < abs_right };

                stack.insert(0, U256::from(result as u8));
            } else {
                // signs are different, convert `left_negative` bool to 1 or 0
                stack.insert(0, U256::from(left_negative as u8));
            }
        }

        // SLT 
        if opcode == 0x13 {
            let right_side = stack.remove(1);
            let left_side = stack.remove(0);
            
            let left_negative = (left_side >> 255) == U256::from(1);
            let right_negative = (right_side >> 255) == U256::from(1);

            if left_negative == right_negative { 
                // handle same sign with absolutes
                let abs_left = if left_negative { (!left_side).overflowing_add(U256::from(1)).0 } else { left_side };
                let abs_right = if right_negative { (!right_side).overflowing_add(U256::from(1)).0 } else { right_side };

                let result = if left_negative { abs_right > abs_left } else { abs_left > abs_right };

                stack.insert(0, U256::from(result as u8));
            } else {
                // signs are different, convert `left_negative` bool to 1 or 0
                stack.insert(0, U256::from(!left_negative as u8));
            }
        }

        // EQ 
        if opcode == 0x14 {
            let right_side = stack.remove(1);
            let left_side = stack.remove(0);
            stack.insert(0, U256::from((right_side == left_side) as u8));
        }

        // ISZERO 
        if opcode == 0x15 {
            let a = stack.remove(0);
            stack.insert(0, U256::from((a == U256::zero()) as u8));
        }

        // NOT 
        if opcode == 0x19 {
            let a = stack.remove(0);
            stack.insert(0, !a);
        }

        // AND 
        if opcode == 0x16 {
            let a = stack.remove(1);
            let b = stack.remove(0);
            stack.insert(0, b & a);
        }

        // OR 
        if opcode == 0x17 {
            let a = stack.remove(1);
            let b = stack.remove(0);
            stack.insert(0, b | a);
        }

        // XOR
        if opcode == 0x18 {
            let a = stack.remove(1);
            let b = stack.remove(0);
            stack.insert(0, b ^ a);
        }

        // SHL 
        if opcode == 0x1B {
            let value = stack.remove(1);
            let shift = stack.remove(0);
            stack.insert(0, if shift > U256::from(255) { U256::zero() } else { value << shift });
        }
        
        // SHR
        if opcode == 0x1C {
            let value = stack.remove(1);
            let shift = stack.remove(0);
            stack.insert(0, if shift > U256::from(255) { U256::zero() } else { value >> shift });
        }

        // SAR
        if opcode == 0x1D {
            let value = stack.remove(1);
            let shift = stack.remove(0);

            // Check if the input value is negative (MSB is 1)
            let value_negative = (value >> 255) == U256::from(1);
            
            if shift > U256::from(255) {
                // If shift is > 255, result is 0 for positive numbers
                // or all 1s (-1) for negative numbers
                stack.insert(0, if value_negative { U256::MAX } else { U256::zero() });
            } else {
                let shift = shift.as_u32();
                let mut result = value >> shift;
                
                // For negative numbers, we need to fill the upper bits with 1s
                if value_negative {
                    // Create a mask with 1s in the positions we shifted
                    let mask = (!U256::zero()) << (256 - shift);
                    result = result | mask;
                }
                
                stack.insert(0, result);
            }
        }
    }



    EvmResult {
        stack,
        success: true,
    }
}
