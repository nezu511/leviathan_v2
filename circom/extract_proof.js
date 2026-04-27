const fs = require("fs");

// ファイルの読み込み（パスは環境に合わせて調整してください）
const proofPath = "./proof.json";
const publicPath = "./public.json";

if (!fs.existsSync(proofPath) || !fs.existsSync(publicPath)) {
    console.error("❌ エラー: proof.json または public.json が見つかりません。パスを確認してください。");
    process.exit(1);
}

const proof = JSON.parse(fs.readFileSync(proofPath));
const pub = JSON.parse(fs.readFileSync(publicPath));

// 数値を 32バイト(64文字) の 16進数文字列に変換し、"0x" を取り除く関数
function toHex32(str) {
    return BigInt(str).toString(16).padStart(64, '0');
}

console.log("\n=== 🌟 Rust テスト用 Hex データ抽出 ===\n");

// 1. Proof (A, B, C) - 合計 256バイト
// Point A: G1 (64 bytes)
const ax = toHex32(proof.pi_a[0]);
const ay = toHex32(proof.pi_a[1]);

// Point B: G2 (128 bytes) - [ [Im, Re], [Im, Re] ] の順に並び替える
// snarkjs の出力は [ [Re, Im], [Re, Im] ] なのでスワップが必要
const bx_re = toHex32(proof.pi_b[0][0]);
const bx_im = toHex32(proof.pi_b[0][1]);
const by_re = toHex32(proof.pi_b[1][0]);
const by_im = toHex32(proof.pi_b[1][1]);

// Point C: G1 (64 bytes)
const cx = toHex32(proof.pi_c[0]);
const cy = toHex32(proof.pi_c[1]);

const fullProofHex = `${ax}${ay}${bx_im}${bx_re}${by_im}${by_re}${cx}${cy}`;

console.log("--- Proof (256 bytes) ---");
console.log(fullProofHex);
console.log("");

// 2. Public Inputs (3つ分) - 合計 96バイト
let inputsHex = "";
pub.forEach((val, index) => {
    const hex = toHex32(val);
    inputsHex += hex;
    console.log(`Input ${index} (${val}):\n${hex}`);
});

console.log("\n--- Full Public Inputs (96 bytes) ---");
console.log(inputsHex);

console.log("\n======================================\n");
console.log("💡 上記の文字列を tests/ に作成するテストコードの変数値として貼り付けてください。");
