use ark_circom::read_zkey;
use ark_serialize::CanonicalSerialize;
use std::fs::File;
use std::io::Write;
use alloy_primitives::hex;

pub fn build_vk_contract() {
    // 1. ark-circomを使って、整理したzk_setup配下のzkeyバイナリを直接パースする
    let mut file = File::open("/home/nezu/leviathan_v2/vote_0001.zkey")
        .expect("Failed to open zkey file");
    let (params, _) = read_zkey(&mut file)
        .expect("Failed to parse zkey");

    // 2. VerifyingKey を抽出し、非圧縮形式のバイト列に変換
    let vk = params.vk;
    let mut vk_bytes = Vec::new();
    vk.serialize_uncompressed(&mut vk_bytes)
        .expect("Failed to serialize VK");

    // バイト列のサイズ (L) を取得
    let l = vk_bytes.len() as u16;
    let l_high = (l >> 8) as u8;
    let l_low = (l & 0xFF) as u8;

    // 3. EVMのInitコード（コンストラクタ）を手組みする
    // アセンブリ: PUSH2 <L>, PUSH1 0x0E, PUSH1 0x00, CODECOPY, PUSH2 <L>, PUSH1 0x00, RETURN
    let mut init_code = vec![
        0x61, l_high, l_low,  // PUSH2 L        (データのサイズ)
        0x60, 0x0E,           // PUSH1 14       (Initコード自体のサイズ = オフセット)
        0x60, 0x00,           // PUSH1 0        (メモリ上の展開先)
        0x39,                 // CODECOPY       (コード領域からメモリへコピー)
        0x61, l_high, l_low,  // PUSH2 L        (Returnするサイズ)
        0x60, 0x00,           // PUSH1 0        (Returnするメモリ開始位置)
        0xF3,                 // RETURN         (EVMにコードとして定着させる)
    ];

    // 4. Initコードの直後に実データ（VKバイト列）を結合
    init_code.extend(vk_bytes);

    // 5. Hex文字列化して deploy_contract で読める形式で保存
    let hex_string = hex::encode(&init_code);
    let mut out = File::create("/home/nezu/leviathan_v2/solidity/out/VK_Data.bin")
        .expect("Failed to create VK_Data.bin");
    out.write_all(hex_string.as_bytes()).unwrap();

    println!("✅ VK_Data.bin generated! Payload size: {} bytes", l);
}
