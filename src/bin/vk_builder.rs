use ark_circom::read_zkey;
use ark_serialize::CanonicalSerialize;
use std::fs::File;
use std::io::Write;

pub fn main() {
    // 1. zkeyファイルを読み込む (パスはプロジェクトルートからの相対パス)
    let mut file = File::open("circom/voting_final.zkey")
        .expect("Failed to open zkey file. Please check if circom/voting_final.zkey exists.");

    let (params, _) = read_zkey(&mut file).expect("Failed to parse zkey");

    // 2. VerifyingKey を抽出し、非圧縮形式のバイナリに変換
    let vk = params.vk;
    let mut vk_bytes = Vec::new();
    vk.serialize_uncompressed(&mut vk_bytes)
        .expect("Failed to serialize VK");

    // バイト列のサイズ (L) を取得
    let l = vk_bytes.len() as u16;
    let l_high = (l >> 8) as u8;
    let l_low = (l & 0xFF) as u8;

    // 3. EVMのInitコード（コンストラクタ）を構築 (14バイト)
    let mut init_code = vec![
        0x61, l_high, l_low, // PUSH2 L        (データのサイズ)
        0x60, 0x0E, // PUSH1 14       (Initコード自体のサイズ = オフセット)
        0x60, 0x00, // PUSH1 0        (メモリ上の展開先)
        0x39, // CODECOPY       (コード領域からメモリへコピー)
        0x61, l_high, l_low, // PUSH2 L        (Returnするサイズ)
        0x60, 0x00, // PUSH1 0        (メモリ上の戻り値の開始位置)
        0xf3, // RETURN         (メモリの内容をコードとして返して終了)
    ];

    // Initコードの直後に純粋な数学データを結合
    init_code.extend(vk_bytes);

    // 4. 【重要】生のバイナリ形式で保存
    let mut out = File::create("solidity/out/VK_Data.bin").expect("Failed to create VK_Data.bin");

    out.write_all(&init_code)
        .expect("Failed to write binary to file");

    println!(
        "✅ VK_Data.bin generated as RAW BINARY! size: {} bytes",
        init_code.len()
    );
}
