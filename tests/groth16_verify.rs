use alloy_primitives::{U256, hex};
use leviathan_v2::leviathan::leviathan::LEVIATHAN;
use leviathan_v2::leviathan::structs::VersionId;
use leviathan_v2::my_trait::leviathan_trait::MCC;
use leviathan_v2::vk_builder::build_vk_contract; // ビルダーを呼び出す
use std::fs;
use tracing_subscriber::EnvFilter;

#[test]
fn test_direct_my_groth16_call() {
    // 🌟 RUST_LOG ログ表示の初期化
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::DEBUG.into()))
        .try_init();

    tracing::info!("--- ZK-EVM Groth16 Integration Test Start ---");

    // 1. テストの直前にバイナリ形式で VK をビルド
    build_vk_contract();

    // 2. あなたが抽出した Hex データ
    let proof_hex = "03b78978de64acad9534a2273d369feb69dce1980970350b6e8e292ac57a1aae1bbc5f13a67c252569377214a3e215a059068629240a2789c91615d95830faf22ac409a6d0b974c24c1331d8d7ae0b20fba9253bb4bae2522abb7cfc874d7c9c1cd5f721ecb39ec09da879be8dc3e81708d07d75a9fde6043c25a545921d79bd2abb38bada8f1425407d961425fcf0e229cf9ea88098ac649571332bcfd01ac12f2587af9928c79a516589810ba7b0359b2bf94243c73fee8a86ab755b17b9ca256328af00c597792f320ccd5f811de9a796b2a5f3dc4a69ca58e27d50ce97201de2ed4d5bfe3afa700f0872ccd4ccdcc70fc4f8b6bf510a2b1c35e96afaa256";
    let inputs_hex = "096f56a93ef8bcf4f5efc79d0967649f93d08eff0af7dca5a4f9aa8db1a434b61914879b2a4e7f9555f3eb55837243cefb1366a692794a7e5b5b3181fb14b49b0000000000000000000000000000000000000000000000000000000000000001";

    let proof_bytes = hex::decode(proof_hex).unwrap();
    let inputs_bytes = hex::decode(inputs_hex).unwrap();

    // 3. 今作ったバイナリを読み込み、14バイトのヘッダーを飛ばす
    let full_bin = fs::read("solidity/out/VK_Data.bin").expect("VK_Data.bin not found");
    let vk_bytes = &full_bin[14..];
    let vk_len = vk_bytes.len();

    tracing::debug!("Verified Binary VK Length: {} bytes", vk_len);

    // 4. プレコンパイル入力データの構築
    let mut data = Vec::new();
    let len_buf = U256::from(vk_len).to_be_bytes::<32>(); // 正しいバイト変換
    data.extend_from_slice(&len_buf);
    data.extend_from_slice(vk_bytes);
    data.extend_from_slice(&proof_bytes);
    data.extend_from_slice(&inputs_bytes);

    // 5. LEVIATHAN::my_groth16 を実行
    let gas = U256::from(3_000_000); // 十分なガス
    let version = VersionId::Petersburg;
    let result = LEVIATHAN::my_groth16(gas, &data, version);

    // 6. 結果の最終判定
    match result {
        Ok((_, output)) => {
            let success = output[31];
            tracing::info!("Verification Success Signal: {}", success);
            assert_eq!(
                success, 1,
                "数学的検証に失敗しました。座標の順序を再確認してください。"
            );
        }
        Err(_) => panic!("構造的エラー！データの切り出し位置や長さチェックが失敗しています。"),
    }

    tracing::info!("✅ CONGRATULATIONS: Leviathan's Groth16 Precompile is mathematically sound!");
}
