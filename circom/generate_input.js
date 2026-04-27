const { buildPoseidon } = require("circomlibjs");
const fs = require("fs");

async function main() {
    const args = process.argv.slice(2);
    // Rust から渡された Root を取得
    const officialRootHex = args[0] ? "0x" + args[0] : null;

    const poseidon = await buildPoseidon();
    const F = poseidon.F;

    // あなたの秘密情報 (テスト用)
    const secret = "12345";
    const nullifier = "67890";
    const voteChoice = "1";

    const toStr = (buf) => F.toString(buf);

    // Commitment と NullifierHash
    const commitmentBuf = poseidon([secret, nullifier]);
    const nullifierHashBuf = poseidon([nullifier, 1]);

    const levels = 20;

    // 🌟 Rootの決定ロジック (スコープを修正)
    let finalRootStr;
    if (officialRootHex && officialRootHex !== "0x0000000000000000000000000000000000000000000000000000000000000000") {
        finalRootStr = BigInt(officialRootHex).toString();
        console.log("Using Official Root from EVM Slot 22:", finalRootStr);
    } else {
        // Fallback: 自前で計算して回路を納得させる
        let currentHash = F.toObject(commitmentBuf);
        for (let i = 0; i < levels; i++) {
            const nextHashBuf = poseidon([currentHash, 0n]);
            currentHash = F.toObject(nextHashBuf);
        }
        finalRootStr = currentHash.toString();
        console.log("Calculated Root in JS (Fallback):", finalRootStr);
    }

    // 🌟 inputJson をここで定義 (ReferenceError 対策)
    const inputJson = {
        root: finalRootStr,
        nullifierHash: toStr(nullifierHashBuf),
        voteChoice: voteChoice,
        secret: secret,
        nullifier: nullifier,
        pathElements: Array(levels).fill("0"),
        pathIndices: Array(levels).fill(0)
    };

    fs.writeFileSync("input.json", JSON.stringify(inputJson, null, 2));
    console.log("✅ input.json generated successfully!");
}

main().catch(console.error);
