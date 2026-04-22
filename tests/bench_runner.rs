use alloy_primitives::{U256, hex, keccak256};
use leviathan_v2::leviathan::leviathan::LEVIATHAN;
use leviathan_v2::leviathan::structs::VersionId;
use leviathan_v2::my_trait::leviathan_trait::CompiledContract;
use std::fs::OpenOptions;
use std::io::Write;
use std::time::Instant;

// RSA生成用のクレート（main.rs で使用しているものと同じ）
use rand::rngs::OsRng;
use rsa::{RsaPrivateKey, RsaPublicKey, pkcs1v15::Pkcs1v15Sign, traits::PublicKeyParts};
use sha2::Sha256;

#[test]
fn benchmark_rsa_precompile() {
    // ---------------------------------------------------------
    // 1. テストデータの動的生成ロジック
    // ---------------------------------------------------------
    let mut rng = OsRng;

    // 2048ビットの鍵ペアを生成
    let priv_key = RsaPrivateKey::new(&mut rng, 2048).expect("鍵生成に失敗しました");
    let pub_key = RsaPublicKey::from(&priv_key);

    // メッセージハッシュ (main.rs の仕様に準拠)
    let msg_hash = keccak256("Leviathan Benchmarking Payload");

    // 署名の生成 (PKCS#1 v1.5)
    let signature = priv_key
        .sign(Pkcs1v15Sign::new::<Sha256>(), msg_hash.as_slice())
        .expect("署名に失敗しました");

    let n = pub_key.n().to_bytes_be(); // Modulus (256 bytes)
    let e = pub_key.e().to_bytes_be(); // Exponent (可変長)

    // あなたの設計したプレコンパイル形式に合わせてパッキング
    // Signature [256] + Modulus [256] + Hash [32] + Exponent [可変]
    let mut payload = Vec::with_capacity(256 + 256 + 32 + e.len());
    payload.extend_from_slice(&signature);
    payload.extend_from_slice(&n);
    payload.extend_from_slice(msg_hash.as_slice());
    payload.extend_from_slice(&e);

    // ---------------------------------------------------------
    // 2. ベンチマーク実行
    // ---------------------------------------------------------
    let initial_gas = U256::from(10_000_000);
    let version = VersionId::Petersburg;

    // CSVファイルの準備 (gas_analy ディレクトリが必要)
    std::fs::create_dir_all("gas_analy").ok();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("gas_analy/rsa_benchmarks.csv")
        .expect("CSVファイルが開けません");

    // ⏱️ 計測開始
    let start = Instant::now();

    // プレコンパイル関数を直接実行 (EVMのオーバーヘッドを除外)
    let result = LEVIATHAN::my_rsa(initial_gas, &payload, version);

    // ⏱️ 計測終了 (マイクロ秒)
    let elapsed_us = start.elapsed().as_micros();

    // ---------------------------------------------------------
    // 3. 結果の記録
    // ---------------------------------------------------------
    let status = if result.is_ok() { "Success" } else { "Failed" };

    let csv_line = format!(
        "PayloadLen:{},Status:{},Time_us:{}\n",
        payload.len(),
        status,
        elapsed_us
    );
    file.write_all(csv_line.as_bytes()).unwrap();

    println!("✅ Benchmark Result:");
    println!("   Payload Size : {} bytes", payload.len());
    println!("   Execution Time: {} us", elapsed_us);
    println!("   Status       : {}", status);
}
