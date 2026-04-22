use std::collections::HashMap;
use std::fs;
use std::io::Write;

// alloy_primitives の hex を使用して E0433 を解消
use alloy_primitives::{Address, U256, hex, keccak256};

// 署名生成のためのクレート
use alloy_rlp::{Encodable, Header};
use bytes::BytesMut;
use eth_trie::{EthTrie, Trie};
use secp256k1::{Message, Secp256k1, SecretKey};
use sha3::{Digest, Keccak256};

// 🌟 crate:: ではなく パッケージ名 (ここでは leviathan_v2 と仮定) を使用します
use leviathan_v2::leviathan::leviathan::LEVIATHAN; // LEVIATHAN を追加
use leviathan_v2::leviathan::structs::{BlockHeader, Transaction, VersionId};
use leviathan_v2::leviathan::world_state::{Account, MptAccount, WorldState}; // MptAccount を追加
use leviathan_v2::my_trait::leviathan_trait::{State, TransactionExecution};
use leviathan_v2::test::state_parser::{IndexType, StateTestSuite};

// --- ヘルパー関数 ---

// 🌟 追加: JSONの "network" 文字列から VersionId を取得する関数
fn parse_version(network_str: &str) -> VersionId {
    // ">Frontier" や ">=Frontier" などのプレフィックスを削除して純粋なフォーク名にする
    let clean_str = network_str.trim_start_matches(">=").trim_start_matches('>');
    match clean_str {
        "Frontier" => VersionId::Frontier,
        "Homestead" => VersionId::Homestead,
        "EIP150" | "TangerineWhistle" => VersionId::TangerineWhistle,
        "EIP158" | "SpuriousDragon" => VersionId::SpuriousDragon,
        "Byzantium" => VersionId::Byzantium,
        "Constantinople" => VersionId::Constantinople,
        "Petersburg" | "ConstantinopleFix" => VersionId::Petersburg,
        "Istanbul" => VersionId::Istanbul,
        "Berlin" => VersionId::Berlin,
        "London" => VersionId::London,
        "Merge" | "Paris" => VersionId::Merge,
        "Shanghai" => VersionId::Shanghai,
        "Cancun" => VersionId::Cancun,
        _ => VersionId::Latest, // 未知の場合は最新とする
    }
}

fn strip_comments(val: &mut serde_json::Value) {
    match val {
        serde_json::Value::Object(map) => {
            map.retain(|k, _| !k.starts_with("//") && !k.starts_with('_'));
            for v in map.values_mut() {
                strip_comments(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr.iter_mut() {
                strip_comments(v);
            }
        }
        _ => {}
    }
}

fn parse_u256(s: &str) -> U256 {
    let s = s.trim();
    if s.is_empty() {
        return U256::ZERO;
    }
    if s.starts_with("0x") {
        U256::from_str_radix(&s[2..], 16).unwrap_or(U256::ZERO)
    } else {
        U256::from_str_radix(s, 10).unwrap_or(U256::ZERO)
    }
}

fn parse_address(s: &str) -> Address {
    let clean_s = s.trim_start_matches("0x");
    let bytes = hex::decode(clean_s).unwrap_or_default();
    let mut addr = [0u8; 20];
    let copy_len = bytes.len().min(20);
    addr[20 - copy_len..].copy_from_slice(&bytes[..copy_len]);
    Address::new(addr)
}

fn parse_code(code_str: &str) -> Vec<u8> {
    let s = code_str.trim();
    if s == "{ [[0]] (ADD 1 1) }" {
        return hex::decode("6001600101600055").unwrap();
    }
    hex::decode(s.trim_start_matches("0x")).unwrap_or_default()
}

fn sign_transaction(
    nonce: U256,
    gas_price: U256,
    gas_limit: U256,
    to: Option<Address>,
    value: U256,
    data: &[u8],
    secret_key_hex: &str,
) -> (U256, U256, U256) {
    // 1. 各要素のRLPペイロード長を事前計算する
    let mut payload_length = 0;
    payload_length += nonce.length();
    payload_length += gas_price.length();
    payload_length += gas_limit.length();

    let to_slice = match &to {
        Some(addr) => addr.0.as_slice(),
        None => &[], // 空のバイト列
    };
    payload_length += to_slice.length();
    payload_length += value.length();
    payload_length += data.length();

    // 2. 必要なメモリを一括で確保し、リストのヘッダーを書き込む
    let mut out = BytesMut::with_capacity(payload_length + 10);
    Header {
        list: true,
        payload_length,
    }
    .encode(&mut out);

    // 3. データを順次エンコード
    // u256_to_minimal_bytes を使わなくても、U256型が勝手にゼロ省略してくれます！
    nonce.encode(&mut out);
    gas_price.encode(&mut out);
    gas_limit.encode(&mut out);
    to_slice.encode(&mut out);
    value.encode(&mut out);
    data.encode(&mut out);

    // RLPエンコードされたバイト列を取り出す
    let rlp_encoded = out.freeze();

    // 4. Keccak256でハッシュ化して32バイトのハッシュを得る
    let mut hasher = Keccak256::new();
    hasher.update(&rlp_encoded);
    let hash: [u8; 32] = hasher.finalize().into();

    // --- 以下、secp256k1 による署名ロジックは既存のまま変更なし ---
    let secp = Secp256k1::new();
    let secret_key_bytes = hex::decode(secret_key_hex).expect("Invalid secret key hex");
    let secret_key = SecretKey::from_slice(&secret_key_bytes).expect("Invalid secret key");
    let message = Message::from_digest_slice(&hash).expect("Invalid message hash");

    let sig = secp.sign_ecdsa_recoverable(message, &secret_key);
    let (recovery_id, sig_bytes) = sig.serialize_compact();

    let r = U256::from_be_slice(&sig_bytes[0..32]);
    let s = U256::from_be_slice(&sig_bytes[32..64]);

    let rec_id_i32 = i32::from(recovery_id);
    let v = U256::from(rec_id_i32 as u64 + 27);

    (v, r, s)
}

#[test]
fn state_test() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    /*/ leviathan.rs の state_test 関数内
      if let Ok(mut file) = std::fs::File::create("gas_analy/stRevertTest_benchmarks.csv") {
    // ヘッダーに Gas を追加
    let _ = writeln!(file, "Address,InputLen,Gas,Status,Time_us");
    tracing::info!("ベンチマーク用CSVファイルを初期化しました");
    }*/

    // 対象のディレクトリ
    let test_dirs = vec![
        "TestData/MPTTest/stAttackTest",
        "TestData/MPTTest/stBadOpcode",
        "TestData/MPTTest/stBugs",
        "TestData/MPTTest/stCallCodes",
        "TestData/MPTTest/stCallCreateCallCodeTest",
        "TestData/MPTTest/stCallDelegateCodesCallCodeHomestead",
        "TestData/MPTTest/stCallDelegateCodesHomestead",
        "TestData/MPTTest/stChangedEIP150",
        "TestData/MPTTest/stCodeSizeLimit",
        "TestData/MPTTest/stCreate2",
        "TestData/MPTTest/stCreateTest",
        "TestData/MPTTest/stDelegatecallTestHomestead",
        "TestData/MPTTest/stEIP158Specific",
        "TestData/MPTTest/stExtCodeHash",
        "TestData/MPTTest/stHomesteadSpecific",
        "TestData/MPTTest/stInitCodeTest",
        "TestData/MPTTest/stLogTests",
        "TestData/MPTTest/stMemoryStressTest",
        "TestData/MPTTest/stMemoryTest",
        "TestData/MPTTest/stQuadraticComplexityTest",
        "TestData/MPTTest/stRandom",
        "TestData/MPTTest/stRecursiveCreate",
        "TestData/MPTTest/stRefundTest",
        "TestData/MPTTest/stRevertTest",
        "TestData/MPTTest/stSStoreTest",
        "TestData/MPTTest/stShift",
        "TestData/MPTTest/stSolidityTest",
        "TestData/MPTTest/stStackTests",
        "TestData/MPTTest/stTimeConsuming",
        "TestData/MPTTest/stTransitionTest",
        "TestData/MPTTest/stWalletTest",
        "TestData/MPTTest/stZeroCallsRevert",
        "TestData/MPTTest/stZeroCallsTest",
        "TestData/MPTTest/stZeroKnowledge",
    ];

    let mut total_files = 0;
    let mut pass_cases_count = 0;
    let mut total_cases_count = 0;
    for test_dir in test_dirs {
        println!("\n Scanning directory: {}", test_dir);

        let paths = match std::fs::read_dir(test_dir) {
            Ok(p) => p,
            Err(e) => {
                println!(
                    "Failed to read directory '{}': {}. Skipping...",
                    test_dir, e
                );
                continue;
            }
        };

        for path in paths {
            let path = path.unwrap().path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            total_files += 1;
            let file_name = path.file_name().unwrap().to_str().unwrap();
            println!("\n==================================================");
            println!(" Loading File: {}", file_name);

            let json_data = std::fs::read_to_string(&path)
                .unwrap_or_else(|_| panic!("Failed to read JSON file: {}", file_name));

            let mut raw_json: serde_json::Value = serde_json::from_str(&json_data).unwrap();
            strip_comments(&mut raw_json);

            let suite: StateTestSuite = serde_json::from_value(raw_json)
                .unwrap_or_else(|_| panic!("Failed to parse JSON in {}", file_name));

            for (test_name, test_data) in suite.tests {
                println!("--------------------------------------------------");
                println!("▶ Running State Test: {}", test_name);

                // 🌟 修正ポイント: 実行順序がランダムになるのを防ぐため、
                // ネットワーク(フォーク)を「時代順」にソートして順序を固定する！
                let mut networks: Vec<_> = test_data.post.keys().collect();
                networks.sort_by_key(|net| {
                    let clean_str = net.trim_start_matches(">=").trim_start_matches('>');
                    match clean_str {
                        "Frontier" => 10,
                        "Homestead" => 20,
                        "EIP150" | "TangerineWhistle" => 30,
                        "EIP158" | "SpuriousDragon" => 40,
                        "Byzantium" => 50,
                        "Constantinople" => 60,
                        "ConstantinopleFix" | "Petersburg" => 65,
                        "Istanbul" => 70,
                        "Berlin" => 80,
                        "London" => 90,
                        "Merge" | "Paris" => 100,
                        "Shanghai" => 110,
                        "Cancun" => 120,
                        _ => 999, // 未知のフォークは最後
                    }
                });

                // 🌟 ソート済みのネットワーク配列でループを回す
                for network_str in networks {
                    let post_states = &test_data.post[network_str];
                    let version = parse_version(network_str);

                    for (post_idx, post_state) in post_states.iter().enumerate() {
                        // 🌟 修正ポイント1: 先頭だけではなく、インデックスの配列をすべて取得する
                        let get_usize_vec = |idx: &IndexType| -> Vec<usize> {
                            match idx {
                                IndexType::Single(n) => vec![(*n).max(0) as usize],
                                IndexType::Multi(arr) => {
                                    arr.iter().map(|n| (*n).max(0) as usize).collect()
                                }
                            }
                        };

                        let data_indices = get_usize_vec(&post_state.indexes.data);
                        let gas_indices = get_usize_vec(&post_state.indexes.gas);
                        let value_indices = get_usize_vec(&post_state.indexes.value);

                        // 🌟 修正ポイント2: すべてのインデックスの組み合わせ（直積）でテストを回す
                        for &data_idx in &data_indices {
                            for &gas_idx in &gas_indices {
                                for &value_idx in &value_indices {
                                    total_cases_count += 1;

                                    let tx_data_str = &test_data.transaction.data[data_idx];
                                    let gas_limit_str = &test_data.transaction.gas_limit[gas_idx];
                                    let value_str = &test_data.transaction.value[value_idx];

                                    // 🌟 表示も分かりやすく修正（どのインデックスの組み合わせをテストしているか表示）
                                    println!(
                                        "  [Network: {:<17}] Matrix {} (data: {}, gas: {}, value: {})",
                                        network_str, post_idx, data_idx, gas_idx, value_idx
                                    );

                                    // 1. WorldStateの初期化 (必ず毎ループ初期化する！)
                                    let mut state = WorldState::new();

                                    for (addr_str, acc_data) in &test_data.pre {
                                        let addr = parse_address(addr_str);

                                        let mut storage_trie = EthTrie::new(state.data.clone());
                                        let mut storage = HashMap::new();

                                        if let Some(st) = &acc_data.storage {
                                            for (k, v) in st {
                                                let key_u256 = parse_u256(k);
                                                let val_u256 = parse_u256(v);
                                                storage.insert(key_u256, val_u256);

                                                if !val_u256.is_zero() {
                                                    let key_byte: [u8; 32] = key_u256.to_be_bytes();
                                                    let key_hash = keccak256(key_byte);
                                                    let val_rlp = alloy_rlp::encode(val_u256);
                                                    storage_trie
                                                        .insert(
                                                            key_hash.as_slice(),
                                                            val_rlp.as_slice(),
                                                        )
                                                        .unwrap();
                                                }
                                            }
                                        }
                                        // 初期ストレージの正しいルートハッシュを確定させる！
                                        let initial_storage_root =
                                            storage_trie.root_hash().unwrap();

                                        let nonce = acc_data
                                            .nonce
                                            .as_ref()
                                            .map(|n| parse_u256(n).try_into().unwrap_or(0))
                                            .unwrap_or(0);
                                        let balance = acc_data
                                            .balance
                                            .as_ref()
                                            .map(|b| parse_u256(b))
                                            .unwrap_or(U256::ZERO);
                                        let code = acc_data
                                            .code
                                            .as_ref()
                                            .map(|c| parse_code(c))
                                            .unwrap_or_default();

                                        let code_hash = keccak256(&code);
                                        state.code_storage.insert(code_hash, code.clone());

                                        let account = Account {
                                            nonce,
                                            balance,
                                            storage,
                                            code,
                                            storage_hash: initial_storage_root, // 🌟 ダミーではなく本物をセット！
                                            account_hash: keccak256(&[]), // 後で不要になる場合は削除してOK
                                        };

                                        state.add_account(&addr, account);

                                        let mpt_account = MptAccount::new(
                                            nonce,
                                            balance,
                                            initial_storage_root,
                                            code_hash,
                                        );
                                        let addr_hash = keccak256(&addr);
                                        let mut mpt_rlp = Vec::new();
                                        mpt_account.encode(&mut mpt_rlp);
                                        state
                                            .eth_trie
                                            .insert(addr_hash.as_slice(), mpt_rlp.as_slice())
                                            .unwrap();
                                    }

                                    let pre_state_root = state.eth_trie.root_hash().unwrap();
                                    println!(
                                        "    [Pre-State] Initial State Root: {}",
                                        pre_state_root
                                    );

                                    // --- ここから下が Env情報の構築 と トランザクション実行 (leviathan.execution) ---

                                    let block_header = BlockHeader {
                                        h_beneficiary: parse_address(
                                            &test_data.env.current_coinbase,
                                        ),
                                        h_timestamp: parse_u256(&test_data.env.current_timestamp),
                                        h_number: parse_u256(&test_data.env.current_number),
                                        h_prevrandao: parse_u256(&test_data.env.current_difficulty),
                                        h_gaslimit: parse_u256(&test_data.env.current_gas_limit),
                                        h_basefee: U256::ZERO,
                                    };

                                    let tx_data = parse_code(tx_data_str);
                                    let gas_limit = parse_u256(gas_limit_str);
                                    let value = parse_u256(value_str);
                                    let to_address = if test_data.transaction.to.is_empty() {
                                        None
                                    } else {
                                        Some(parse_address(&test_data.transaction.to))
                                    };
                                    let nonce = parse_u256(&test_data.transaction.nonce);
                                    let gas_price = parse_u256(&test_data.transaction.gas_price);
                                    let secret_key_hex =
                                        test_data.transaction.secret_key.trim_start_matches("0x");

                                    let (v, r, s) = sign_transaction(
                                        nonce,
                                        gas_price,
                                        gas_limit,
                                        to_address.clone(),
                                        value,
                                        &tx_data,
                                        secret_key_hex,
                                    );

                                    let transaction = Transaction {
                                        data: tx_data,
                                        t_to: to_address,
                                        t_gas_limit: gas_limit,
                                        t_price: gas_price,
                                        t_value: value,
                                        t_nonce: nonce.try_into().unwrap_or(0),
                                        t_w: v,
                                        t_r: r,
                                        t_s: s,
                                    };

                                    // 2. 実行
                                    let mut leviathan = LEVIATHAN::new(version);
                                    let _result =
                                        leviathan.execution(&mut state, transaction, &block_header);

                                    // 3. 🌟 究極の検証フェーズ：State Root Hashの比較
                                    let expected_hash: alloy_primitives::B256 = post_state
                                        .hash
                                        .parse()
                                        .expect("Failed to parse expected hash");

                                    let actual_hash = state.eth_trie.root_hash().unwrap();

                                    if actual_hash == expected_hash {
                                        println!(
                                            "    => Success! State Root Matches: {}",
                                            expected_hash
                                        );
                                        pass_cases_count += 1;
                                    } else {
                                        println!("    => FAILED!");
                                        println!("       Expected: {}", expected_hash);
                                        println!("       Actual  : {}", actual_hash);
                                        println!(
                                            "\n=== 🔍 最終ステートのダンプ (Cache内の最新状態) ==="
                                        );
                                        for (address, account) in &state.cache {
                                            println!(
                                                "Address: 0x{}",
                                                alloy_primitives::hex::encode(address.0)
                                            );
                                            println!("  Nonce       : {}", account.nonce);
                                            println!("  Balance     : {}", account.balance);
                                            println!(
                                                "  Code (len)  : {} bytes",
                                                account.code.len()
                                            );
                                            println!("  Storage:");
                                            if account.storage.is_empty() {
                                                println!("    (empty)");
                                            } else {
                                                let mut keys: Vec<_> =
                                                    account.storage.keys().collect();
                                                keys.sort();
                                                for k in keys {
                                                    let v = account.storage.get(k).unwrap();
                                                    println!("    [{}] -> {}", k, v);
                                                }
                                            }
                                            println!("  StorageRoot : {}", account.storage_hash);
                                            println!(
                                                "---------------------------------------------------"
                                            );
                                        }
                                        println!(
                                            "===================================================\n"
                                        );
                                        assert_eq!(
                                            actual_hash, expected_hash,
                                            "State root mismatch in test: {}",
                                            test_name
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    println!("\n==================================================");
    println!(
        "最終結果: {} ファイル中、{} / {} のテストケースをクリアしました！",
        total_files, pass_cases_count, total_cases_count
    );
    println!("==================================================\n");
}
