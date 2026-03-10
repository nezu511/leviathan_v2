#![allow(dead_code)]

use primitive_types::U256;
use crate::evm::gas_struct::Gas;
use crate::world_state::world_state_struct::{WorldState, Address,Action};
use crate::world_state::leviathan_struct::{EvmContext};
use crate::world_state::account_struct::Account;
use crate::world_state::transaction_struct::InternalTransaction;
use crate::world_state::leviathan_struct::Leviathan;


#[derive(Debug)]
pub struct EVM <'a>{
    pc: usize, //プログラムカウンター
    code: Vec<u8>,
    stack:Vec<U256>,
    gas_limit:&'a mut Gas,
    memory:Vec<u8>,
    evmcontext: &'a mut EvmContext,
    world_state: &'a mut WorldState,
    child_number: usize,
}

impl<'a> EVM<'a> {
    //codeに外部から変更を加えたくない
    //所有権をEVMに渡す
    pub fn new(evmcontext:&'a mut EvmContext, world_state: &'a mut WorldState, gas_limit: &'a mut Gas)  -> Self{
        let pc = 0;
        let child_number = evmcontext.child_number + 1;
        let code = evmcontext.code.clone();
        let mut memory = Vec::new();

        let stack: Vec<U256>  = Vec::with_capacity(1024);

        Self {pc, code, stack, gas_limit, memory, evmcontext, world_state, child_number}
    }

    pub fn push(&mut self, val: U256) -> Result<(), &'static str> {
        //現在の要素数を取得, 1024と比較
        if 1024 <= self.stack.len() {
            return Err("【例外】1024以上の要素のスタックはできません");
        }
        self.stack.push(val);
        Ok(())
    }
    
    pub fn pop(&mut self) -> Result<U256, &'static str> {
        self.stack.pop().ok_or("【例外】Stack underflow")
    }

    pub fn read_storage(&mut self, key:&U256) -> Result<U256,&'static str>{
        let address:&mut Address = &mut self.evmcontext.address;
        let myself_account: &mut Account = &mut self.world_state.is_stated(&address).unwrap();
        let value = myself_account.storage_hash.get(&key);
        let result = match value {
            None => U256::from(0),
            Some(&x) => x.clone(),
        };
        return Ok(result)
    }

    fn scan_code(&self) -> Vec<u8>{
        let code_len = self.code.len();
        let mut valid_JUMPDEST:Vec<u8> = vec![0;code_len];
        let mut pointer = 0usize;
        while pointer < code_len {
            let x = self.code[pointer];
            match x {
                0x5b => {
                    valid_JUMPDEST[pointer] = 1;
                    pointer += 1;
                },
                0x60 ..=0x7f => {
                    let val = x as usize;
                    pointer += (val - 0x60usize) + 2usize;
                },
                _ => pointer +=1,
            }
        }
        return valid_JUMPDEST;
    }



    pub fn run(&mut self)  -> Result<Vec<u8>, &'static str> {
        println!("【開始時】ガス使用量 {:?} gas", self.gas_limit.meter_reading());
        let valid_jumpdest = self.scan_code();
        while self.pc < self.code.len() {
            let com = self.code[self.pc];
            self.gas_limit.consumption(com)?;
            match com  {
                0x00 => return Ok(Vec::new()),  //END

                0x01 => {   //Add
                    let val1 = self.pop()?;
                    let val2 = self.pop()?;
                    self.push(val1 + val2)?;
                    self.pc += 1;
                },

                0x30 => {   //ADDRESS コントラクト自身のアドレスをスタックにpush
                    let address = U256::from_big_endian(&self.evmcontext.address.address[0..20]);
                    self.push(address)?;
                    self.pc +=1;
                },

                0x32 => {   //ORIGIN
                    let address = U256::from_big_endian(&self.evmcontext.origin.address[0..20]);
                    self.push(address)?;
                    self.pc +=1;
                },

                0x33 => {   //CALLER
                    let address = U256::from_big_endian(&self.evmcontext.sender.address[0..20]);
                    self.push(address)?;
                    self.pc +=1;
                },

                0x34 => {   //CALLVALUE
                    self.push(self.evmcontext.eth)?;
                    self.pc +=1;
                },

                0x35 => {   //CALLDATALOAD
                    let pointer = self.pop()?.as_usize();
                    let mut buffer = [0u8; 32];
                    let data_len = self.evmcontext.data.len();
                    if pointer <  data_len {
                        let copy_len = std::cmp::min(32, data_len - pointer);
                        buffer[..copy_len].copy_from_slice(&self.evmcontext.data[pointer ..pointer + copy_len]);
                    }
                    let data = U256::from_big_endian(&buffer);
                    self.push(data)?;
                    self.pc +=1;
                },

                0x36 => {   //CALLDATASIZE
                    self.push(U256::from(self.evmcontext.data.len()))?;
                    self.pc +=1;
                },


                0x37 => {   //CALLDATACOPY
                    let destOffset = self.pop()?.as_usize();
                    let offset = self.pop()?.as_usize();
                    let size = self.pop()?.as_usize();
                    self.gas_limit.gass_copy(size)?;    //コピー処理の追加ガスを請求
                    if size != 0 {
                        let required_size = destOffset + size;
                        if required_size > self.memory.len() {
                            let words =  (required_size + 31) /32;
                            self.gas_limit.gass_memory(self.memory.len(), words)?;  //メモリ拡張ガス
                            self.memory.resize(words * 32, 0);
                        }
                        let data_len = self.evmcontext.data.len();
                        let copy_len = if offset < data_len {
                            std::cmp::min(size, data_len - offset)
                        } else {
                            0
                        };
                        if copy_len >0 {
                            self.memory[destOffset .. destOffset + copy_len ].copy_from_slice(&self.evmcontext.data[offset .. copy_len]);
                        }
                        if size > copy_len {
                            self.memory[destOffset + copy_len .. destOffset + size].fill(0);
                        }
                    }
                    self.pc +=1;
                },

                0x39 => {   //CODECOPY
                    let destOffset = self.pop()?.as_usize();
                    let offset = self.pop()?.as_usize();
                    let size = self.pop()?.as_usize();
                    self.gas_limit.gass_copy(size)?;    //コピー処理の追加ガスを請求

                    if size > 0 {
                        let required_size = destOffset + size;
                        if required_size > self.memory.len() {
                            let words = (required_size + 31) /32;
                            self.gas_limit.gass_memory(self.memory.len(), words)?;  //メモリ拡張ガス
                            self.memory.resize(words * 32, 0);
                        }

                        let data_len = self.evmcontext.code.len();
                        let copy_len = if offset < data_len {
                            std::cmp::min(size, data_len - offset)
                        }else{
                            0
                        };

                        if copy_len > 0 {
                            self.memory[destOffset .. destOffset + copy_len].copy_from_slice(&self.evmcontext.code[offset .. offset + copy_len]);
                        }

                        if size > copy_len {
                            self.memory[destOffset + copy_len ..destOffset + size].fill(0);
                        }
                    }
                    self.pc +=1;
                },

                0x50 => {//pop
                         self.pop()?;
                         self.pc += 1;
                },

                0x51 => {   //MLOAD メモリから読み込む（32B)
                    let pointer:usize = self.pop()?.as_usize();
                    let required_size = pointer + 32;

                    if required_size > self.memory.len() {
                        let words = (required_size + 31) / 32;
                        self.gas_limit.gass_memory(self.memory.len(), words)?;  //メモリ拡張ガス
                        self.memory.resize(words * 32, 0);
                    }
                    let slice = &self.memory[pointer .. required_size];
                    let data = U256::from_big_endian(slice);
                    self.push(data)?;
                    self.pc +=1
                },

                0x52 => {   //MSTORE メモリに保存(32)
                    let pointer = self.pop()?.as_usize();
                    let data = self.pop()?;
                    let required_size = pointer + 32;
                    if required_size > self.memory.len() {
                        let words = (required_size + 31) / 32;
                        self.gas_limit.gass_memory(self.memory.len(), words)?;  //メモリ拡張ガス
                        self.memory.resize(words * 32, 0)
                    }
                    let slice = &mut self.memory[pointer .. required_size];
                    let bytes = data.to_big_endian();
                    slice.copy_from_slice(&bytes);
                    self.pc +=1
                },

                0x53 => {   //MSTORE8
                    let pointer = self.pop()?.as_usize();
                    let data = self.pop()?;
                    let required_size = pointer + 1;
                    if required_size > self.memory.len() {
                        let words = (required_size + 31) /32;
                        self.gas_limit.gass_memory(self.memory.len(), words)?;  //メモリ拡張ガス
                        self.memory.resize(words * 32,0)
                    }
                    let slice = &mut self.memory[pointer..required_size];
                    let bytes = data.to_big_endian();
                    slice.copy_from_slice(&bytes[31..32]);
                    self.pc +=1
                },

                0x54 => {   //SLOAD
                    let key:U256 = self.pop()?;
                    let value = self.read_storage(&key)?;
                    self.push(value)?;
                    self.pc +=1;
                },
                
                0x55 => {   //SSTORE
                    let key = self.pop()?;      //キー
                    let value = self.pop()?;    //保存したい値
                    let address:&mut Address = &mut self.evmcontext.address;
                    let myself_account: &mut Account = &mut self.world_state.is_stated(&address).unwrap();
                    if myself_account.storage_hash.contains_key(&key) { //キーが見つかるか
                        let pre_value = myself_account.storage_hash.get(&key);
                        let pre_value = pre_value.unwrap().clone();
                        if value.is_zero(){
                            self.gas_limit.use_gas(U256::from(4800))?;
                            self.gas_limit.refund_gas(U256::from(4800));
                            myself_account.storage_hash.remove(&key);
                        }else{
                            //上書き（同じかどうか確認)
                            if pre_value != value {
                                self.gas_limit.use_gas(U256::from(4800))?;
                                myself_account.storage_hash.insert(key,value);
                            }
                        }
                        self.world_state.push_acction(Action::Sstorage(self.evmcontext.address.clone(), pre_value,key));
                    }else{
                        let pre_value = U256::from(0);
                        if !value.is_zero() {   //初key-, valueは0以外　書き込みもしくは値の確認？
                            self.gas_limit.use_gas(U256::from(19800))?;
                            myself_account.storage_hash.insert(key,value);
                        self.world_state.push_acction(Action::Sstorage(self.evmcontext.address.clone(), pre_value,key));
                        }
                    }
                    self.pc +=1;
                },

                0x56 => {   //JUMP
                    let destination = self.pop()?.as_usize();
                    if 1 == valid_jumpdest[destination] {
                        self.pc = destination;
                    }else{
                        return Err("【例外】JUMP先がJUMPDESTではない");
                    }
                },
                
                0x57 => {       //JUMPI:条件付きJUMP
                    let dest = self.pop()?.as_usize(); //飛び先の番地
                    let cond = self.pop()?; //条件判定用のフラグ
                    if cond.is_zero() {
                        self.pc +=1
                    }else{
                        if 1 == valid_jumpdest[dest] {
                            self.pc = dest;
                        }else{
                            return Err("【例外】JUMP先がJUMPDESTではない");
                        }
                    }
                },


                0x5b => self.pc += 1,   //JUMPDEST

                0x60 ..=0x7f => {
                    let required_data_len = usize::from((com - 0x60) + 1);
                    let mut buffer = [0u8;32];

                    let data_number = self.code.len() - (self.pc + 1);
                   
                    if required_data_len <= data_number {
                        buffer[(32 - required_data_len)..].copy_from_slice(&self.code[self.pc + 1 ..=self.pc + required_data_len]);
                    }else{
                        if 0 != data_number {
                            let copy_len = std::cmp::min(32, data_number);
                            let start = 32 - required_data_len;
                            buffer[start .. start + copy_len].copy_from_slice(&self.code[self.pc + 1 ..= self.pc + copy_len]);
                        }
                    }
                    let data = U256::from_big_endian(&buffer);
                    self.push(data)?;
                    self.pc += required_data_len + 1;
                },

                0xf1 => {
                    let gas = self.pop()?;
                    let to_address = self.pop()?;   //アドレス型に変換
                    let eth = self.pop()?;
                    let args_offset = self.pop()?.as_usize();   //送りたいデータのoffset
                    let args_size = self.pop()?.as_usize();     //送りたいデータのサイズ
                    let return_offset = self.pop()?.as_usize(); //受け取ったデータを自分のメモリのどこに書くのか
                    let return_size = self.pop()?.as_usize();   //受け取ったデータを自分のメモリのどこに書くのか

                    //残高チェック
                    let myself_account = self.world_state.is_stated(&self.evmcontext.address).unwrap();
                    if myself_account.balance < eth {
                        //call失敗
                        self.push(U256::from(0))?;
                        self.pc +=1;
                        continue
                    }

                    //コールスタックの深さ
                    if self.child_number > 1024 {
                        //call失敗
                        self.push(U256::from(0))?;
                        self.pc +=1;
                        continue
                    }


                    //ガスを分けてもらう
                    let child_gas = match self.gas_limit.divide_gas(gas) {
                        Ok(x) => x,
                        Err(x) => return Err(x),
                    };

                    //アドレス型に変換
                    let buffer = &to_address.to_big_endian()[12..32];
                    let mut tmp = [0u8;20];
                    tmp[0..20].copy_from_slice(&buffer[0..20]);
                    let to_address = Address::new(tmp);

                    //メモリの読み取り
                    let args_required_size = if args_size != 0 {
                        args_offset + args_size
                    }else{
                        0
                    };
                    let return_required_size = if return_size != 0 {
                        return_offset + return_size
                    }else{
                        0
                    };
                    let max_required = std::cmp::max(args_required_size, return_required_size);
                    if max_required > self.memory.len() {
                        let words = (max_required + 31) / 32;
                        self.gas_limit.gass_memory(self.memory.len(), words)?;  //メモリ拡張ガス
                        self.memory.resize(words * 32,0)
                    }
                    let data = Vec::from(&self.memory[args_offset .. args_offset + args_size]);

                    //InternalTransactionを作成
                    let internal_transaction = InternalTransaction::new(child_gas,
                                                                        self.evmcontext.address.clone(), 
                                                                        to_address, 
                                                                        self.evmcontext.origin.clone(),
                                                                        eth, data, self.child_number);

                    let chiled_leviathan = Leviathan::internal_execution(internal_transaction, self.world_state);


                    
                    
                }

                0xf3 => {   //RETURN
                    let offset = self.pop()?.as_usize();
                    let size = self.pop()?.as_usize();
                    if size != 0{
                        let mut data = vec![0;size];
                        let required_size = offset + size;
                        if required_size > self.memory.len() {
                            let words = (required_size + 31) /32;
                            self.gas_limit.gass_memory(self.memory.len(), words)?;  //メモリ拡張ガス
                            self.memory.resize(words * 32, 0);
                        }
                        data[..size].copy_from_slice(&self.memory[offset .. offset + size]);
                        return Ok(data);
                    }else{
                        return Ok(Vec::new());
                    }
                },

                _ => return Err("【例外】未定義の命令"),
            }
        }
        Ok(Vec::new())
    }
                    


}

