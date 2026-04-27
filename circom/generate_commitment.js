// circom/generate_commitment.js
const { buildPoseidon } = require("circomlibjs");

async function main() {
    const poseidon = await buildPoseidon();
    const F = poseidon.F;

    // ※ 実際のアプリではユーザーが入力する秘密情報
    const secret = "12345";
    const nullifier = "67890";

    // 葉（Commitment）の計算
    const commitmentBuf = poseidon([secret, nullifier]);
    
    // RustでB256としてパースしやすいように、64文字のHex（16進数）で標準出力する
    let hex = BigInt(F.toString(commitmentBuf)).toString(16);
    console.log(hex.padStart(64, '0'));
}

main().catch(console.error);
