use alloy_primitives::{keccak256, Address, hex};
use rand::Rng;
use secp256k1::{Secp256k1, SecretKey, PublicKey};

fn main() {
    // 1. 乱数生成器の初期化
    let mut rng = rand::thread_rng();
    let secp = Secp256k1::new();

    // 2. 秘密鍵（32バイトの完全な乱数）の生成
    let mut secret_bytes = [0u8; 32];
    rng.fill(&mut secret_bytes);
    
    let secret_key = SecretKey::from_slice(&secret_bytes)
        .expect("秘密鍵の生成に失敗しました");

    // 3. 秘密鍵から公開鍵を導出
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);

    // 4. 非圧縮フォーマットの公開鍵（65バイト）を取得
    // 先頭の1バイト（0x04: 非圧縮を示すプレフィックス）はアドレス計算には使いません
    let uncompressed_pub_key = public_key.serialize_uncompressed();

    // 5. 公開鍵の[1..65]バイトを Keccak256 でハッシュ化
    let hash = keccak256(&uncompressed_pub_key[1..]);

    // 6. ハッシュの「後ろの20バイト（インデックス12から末尾まで）」を切り出してアドレスとする
    let address = Address::from_slice(&hash[12..]);

    // 結果の出力
    println!("--------------------------------------------------");
    println!("Private Key (絶対秘密!) : 0x{}", hex::encode(secret_key.secret_bytes()));
    println!("Address (公開用)        : {}", address.to_string()); // alloyのAddressは自動でEIP-55チェックサム形式になります
    println!("--------------------------------------------------");
}
