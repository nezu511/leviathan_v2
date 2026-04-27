// circom/generate_input.js
const { buildPoseidon } = require("circomlibjs");
const fs = require("fs");

async function main() {
    const args = process.argv.slice(2);
    // 引数: root, myIndex, secret, nullifier, voteChoice, allCommitments(カンマ区切り)
    const officialRootHex = args[0] && args[0] !== "0" ? "0x" + args[0] : null;
    const myIndex = parseInt(args[1] || "0");
    const secret = args[2] || "12345";
    const nullifier = args[3] || "67890";
    const voteChoice = args[4] || "1";
    const allCommitmentsStr = args[5] || "";

    const poseidon = await buildPoseidon();
    const F = poseidon.F;
    const toStr = (buf) => F.toString(buf);

    const nullifierHashBuf = poseidon([nullifier, 1]);
    const levels = 20;
    
    // 文字列のコミットメントリストをBigIntの配列に変換して名簿を再現
    let leaves = allCommitmentsStr.split(",").filter(x => x).map(x => BigInt(x));

    let currentLevelNodes = [...leaves];
    let pathElements = [];
    let pathIndices = [];
    
    let currentIndex = myIndex;
    
    // Solidity側の _insertToTree と全く同じ「疎なMerkle Tree」の計算
    for (let i = 0; i < levels; i++) {
        const isRightNode = currentIndex % 2 === 1;
        pathIndices.push(isRightNode ? 1 : 0);
        
        let siblingIndex = isRightNode ? currentIndex - 1 : currentIndex + 1;
        
        // 🌟 修正: Solidityに合わせて、空ノード（名簿の範囲外）は常に 0n にする！
        let siblingNode = siblingIndex < currentLevelNodes.length ? currentLevelNodes[siblingIndex] : 0n;
        
        pathElements.push(siblingNode.toString());
        
        let nextLevelNodes = [];
        for (let j = 0; j < currentLevelNodes.length; j += 2) {
            let left = currentLevelNodes[j];
            // 🌟 ここも同じく、右側がなければ 0n を入れる
            let right = (j + 1 < currentLevelNodes.length) ? currentLevelNodes[j + 1] : 0n;
            nextLevelNodes.push(F.toObject(poseidon([left, right])));
        }
        currentLevelNodes = nextLevelNodes;
        
        // 🌟 tempZeroのハッシュ化（複雑なロジック）を削除し、シンプルにしました
        currentIndex = Math.floor(currentIndex / 2);
    }
    
    const calculatedRoot = currentLevelNodes[0].toString();
    const finalRootStr = officialRootHex ? BigInt(officialRootHex).toString() : calculatedRoot;

    // デバッグ用: JSとEVMのRootが食い違っていれば警告を出す
    if (officialRootHex && finalRootStr !== calculatedRoot) {
        console.warn(`⚠️ Warning: EVM Root (${finalRootStr}) differs from JS Calculated Root (${calculatedRoot})!`);
    }

    const inputJson = {
        root: finalRootStr,
        nullifierHash: toStr(nullifierHashBuf),
        voteChoice: voteChoice,
        secret: secret,
        nullifier: nullifier,
        pathElements: pathElements,
        pathIndices: pathIndices
    };

    fs.writeFileSync("input.json", JSON.stringify(inputJson, null, 2));
    console.log(`✅ input.json generated for Voter ${myIndex}!`);
}

main().catch(console.error);
