#![allow(dead_code)]

use primitive_types::U256;
use crate::world_state::account_struct::Account;

//インラインのconstではなくグローバル変数のstatic
static GAS_TABLE: [u8; 256] = {
    let mut table = [0; 256];
    
    table[0x00] = 0;    // STOP
    table[0x01] = 3;    // ADD
    table[0x30] = 2;    // ADDRESS
    table[0x32] = 2;    // ORIGIN
    table[0x33] = 2;    // CALLER
    table[0x34] = 2;    // CALLVALUE
    table[0x35] = 3;    // CALLDATALOAD
    table[0x36] = 2;    // CALLDATASIZE
    table[0x37] = 3;    // CALLDATACOPY
    table[0x39] = 3;    // CODECOPY
    table[0x51] = 3;    // MLOAD
    table[0x52] = 3;    // MSTORE
    table[0x53] = 3;    // MSTORE8
    table[0x54] = 200;  // SLOAD
    table[0x55] = 200;  // SSTORE
    table[0x56] = 8;    // JUMP
    table[0x57] = 10;   // JUMPI
    table[0x5b] = 1;    // JUMPDEST
    table[0x60] = 3;    // PUSH1
    table[0xf3] = 0;    // RETURN
    
    let mut i = 0x60;
    while i <= 0x7f {
        table[i] = 3;
        i += 1;
    }

    table   //ブロックの最後は原則返り値
};

#[derive(Debug)]
pub struct Gas{
    amount: U256,
    price: U256,
    used: U256,
    refund: U256,
}

impl Gas {

    pub fn consumption(&mut self, code:u8)  -> Result<(),&'static str>{
        let used_gas = U256::from(GAS_TABLE[code as usize]);
        if self.amount >= used_gas {
            self.amount -= used_gas;
            self.used += used_gas;
            return Ok(());
        }else{
            return Err("ガス不足");
        }
    }

    pub fn gass_copy(&mut self, size:usize) -> Result<(), &'static str> {
        let mut used_gas = U256::from(3 * (size/32));
        if (size % 32) != 0 {
            used_gas += U256::from(3);
        }
        if self.amount >= used_gas {
            self.amount -= used_gas;
            self.used += used_gas;
            return Ok(());
        }else{
            return Err("ガス不足");
        }
    }
    
    //add_wordsは追加予定のワード数
    pub fn gass_memory(&mut self, mem_size:usize, add_words:usize) -> Result<(), &'static str> {
        let mut pre_words =  U256::from(mem_size/32);
        let mut post_words = U256::from(add_words);
        let pre_gass = (U256::from(3) * pre_words) + ((pre_words * pre_words)/512);
        let post_gass = (U256::from(3) * post_words) + ((post_words * post_words)/512);
        let used_gas = post_gass - pre_gass;
        if self.amount >= used_gas {
            self.amount -= used_gas;
            self.used += used_gas;
            return Ok(());
        }else{
            return Err("ガス不足");
        }

    }

    pub fn meter_reading(&self) -> U256{
        self.amount
    }

    pub fn buy_gas(price:U256, gas_limit: usize, account: &mut Account)  -> Self{
        account.sub_balance(price * U256::from(gas_limit));
        let amount = U256::from(gas_limit);
        let used = U256::from(0);
        let refund = U256::from(0);
        Self {amount, price, used, refund}
    }

    pub fn use_gas(&mut self, used_gas:U256)  -> Result<(),&'static str>{
        if self.amount >= used_gas {
            self.amount -= used_gas;
            self.used += used_gas;
            return Ok(());
        }else{
            return Err("ガス不足");
        }
    }

    pub fn refund_gas(&mut self, refund:U256) {
        self.refund += refund;
    }

    pub fn reimburse(&mut self, account: &mut Account) {       
        let price = self.price;
        let amount = self.amount;
        let mut refund_gas = if (self.used / U256::from(5)) > self.refund {
            self.refund
        }else{
            self.used  / U256::from(5)
        };

        refund_gas += amount;
        let eth = refund_gas * self.price;
        account.add_balance(eth);

    }

    pub fn divide_gas(&mut self, part:U256) -> Result<Self, &'static str>{
        if self.amount >= part {
            self.amount -= part;
            let used = U256::from(0);
            let refund = U256::from(0);
            return Ok(Self {amount: part, price:self.price, used, refund});
        }else{
            return Err("ガス不足");
        }
    }


}
        
        
        

