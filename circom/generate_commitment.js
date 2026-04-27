// circom/generate_commitment.js
const { buildPoseidon } = require("circomlibjs");

async function main() {
    const poseidon = await buildPoseidon();
    const F = poseidon.F;

    // 引数から秘密情報を受け取る（なければデフォルト値）
    const secret = process.argv[2] || "12345";
    const nullifier = process.argv[3] || "67890";

    const commitmentBuf = poseidon([secret, nullifier]);
    
    let hex = BigInt(F.toString(commitmentBuf)).toString(16);
    console.log(hex.padStart(64, '0'));
}

main().catch(console.error);
