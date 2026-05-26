#!/bin/bash

# エラーが発生した場合は即座にスクリプトを停止する
set -e

echo "====================================================="
echo " Leviathan: 無人市役所システム E2E テストスクリプト "
echo "====================================================="

# --- 基本設定 ---
RPC_URL="http://127.0.0.1:8545"
PRIVATE_KEY="0x80c58089c4343be9bd0ae0d2af81c615211d1e354a4c6073c9a1c32840f6274a"
GAS_ARGS="--legacy --gas-limit 30000000 --gas-price 1"
echo "Step 1: IdentityRegistry のデプロイ..."

# 締め切りブロックを 1000000 に設定してエンコード
REGISTRY_ARGS=$(cast abi-encode "constructor(uint256)" 1000000 | sed 's/^0x//')
REGISTRY_BYTECODE=$(cat solidity/out/IdentityRegistry.bin | sed 's/^0x//' | tr -d '\n')
IDENTITY_ADDR=$(cast send --rpc-url $RPC_URL --private-key $PRIVATE_KEY $GAS_ARGS \
  --create "0x${REGISTRY_BYTECODE}${REGISTRY_ARGS}" \
  | grep 'contractAddress' | awk '{print $2}')
echo "✅ IdentityRegistry デプロイ完了: ${IDENTITY_ADDR}"

echo "Step 2: VK_Data のデプロイ..."
VK_HEX=$(xxd -p -c 999999 solidity/out/VK_Data.bin | tr -d '\n')
VK_ADDR=$(cast send --rpc-url $RPC_URL --private-key $PRIVATE_KEY $GAS_ARGS \
  --create "0x${VK_HEX}" \
  | grep 'contractAddress' | awk '{print $2}')
echo "✅ VK_Data デプロイ完了: ${VK_ADDR}"

echo "Step 3: Voting のデプロイ..."
# 🌟 修正: IDENTITY_ADDR が新しくなっているので Voting のデプロイもそのまま実行
ARGS=$(cast abi-encode "constructor(address,address)" $VK_ADDR $IDENTITY_ADDR | sed 's/^0x//')
BYTECODE=$(cat solidity/out/Voting.bin | sed 's/^0x//' | tr -d '\n')
VOTING_ADDR=$(cast send --rpc-url $RPC_URL --private-key $PRIVATE_KEY $GAS_ARGS \
  --create "0x${BYTECODE}${ARGS}" \
  | grep 'contractAddress' | awk '{print $2}')
echo "✅ Voting デプロイ完了: ${VOTING_ADDR}"

echo "-----------------------------------------------------"

echo "Step 4: 市民登録 (ZK-SNARKs Proof 送信)..."
# 生成済みの登録用ペイロードを送信
cast send $IDENTITY_ADDR "register(bytes,bytes,bytes,bytes32)" \
  0xae7e3b234804743f52668ec81c4f2bdcc4eb4ba4ada4cf8ddc7ed97a988d4ef9a150ad07d7cccb148b32fd117cd521d2d07e06d58d668266c9cfcebf9430d5a9302ee0ef224e1a36b17be0ef09357dd54284359b9126bca824c1d56f82dce0b1e84cee2ffa4a899e43103a4ed18bb81bf00e007d09e932d8ea72d8c22bb12c394b05e572b345ab995c9c72e8823b47003cad4f71c6b30a40f991618ce32938a818db5108c13995e0a56dd12e9686b820974385ecca0526cc3348e008a23e30b7b9d8bcbe79b39ee4b1751c21529daf675f0dc2c7683372961bc41988a5de24a58ae5bf18b6885d66a671174bf5b977ecf479dd74c5086fe7f2bb1d70f6bbace5 \
  0x010001 \
  0x5abaae92aa9a1af366712e7d37e39d2b6868997e052b6b56837f44c4caf70cbc5fdbf9a87c246c3bda6a24ee9e0feaa4b44c375e27ff5f15b02f28103cc20735bf90c6f6b9bdfb5c9496505097f03c1a1d95348e1fde594bc033e458246794dcde780a2d0e9194f00ea9e1fa2289596b0be50085adbe0ac914a2ddebd6b5b2e118ce0ac402f9f120c8b6ed4a35e1d320e3839fce271a34240ba55e65f6a9537fdb100bb001faffad89ea77a3da7c346b83a6fba59da6873f45f871a8976b745da2e95e5a63aee02353473e9e793bba170d9170ac4e42711d0ede862f7d413addffe7a220f6f2be11457e83b7aaee4bb501eeafcbf8d81a264291522738d6e968 \
  0x0d9bd617a15767818914c1f4cd17a015fa6369d9737093583476763b33472b74 \
  --rpc-url $RPC_URL --private-key $PRIVATE_KEY $GAS_ARGS > /dev/null
echo "✅ 市民登録完了"

# IdentityRegistry から出たログ（イベント）を検索する
cast logs --address $IDENTITY_ADDR --rpc-url $RPC_URL

echo "登録状態の確認 (isRegistered)..."
# コミットメント(0x0d9b...)を渡して登録確認
IS_REG=$(cast call $IDENTITY_ADDR "isRegistered(bytes32)(bool)" 0x0d9bd617a15767818914c1f4cd17a015fa6369d9737093583476763b33472b74 --rpc-url $RPC_URL)
echo "   結果: $IS_REG"

echo "MPT Root Hash の取得..."
CURRENT_ROOT=$(cast call $IDENTITY_ADDR "currentRoot()(bytes32)" --rpc-url $RPC_URL)
echo "   現在のルート: $CURRENT_ROOT"

echo "-----------------------------------------------------"

echo "Step 5: 匿名投票の実行 (ZK-SNARKs Proof 送信)..."
cast send $VOTING_ADDR "castVote(bytes,bytes32,bytes32,uint256)" \
  0x0dabab04839f746082e770368df6ea9b6ffc494613d621ad6722e06e2198935c29bdf89d6ce2ad353057e18fb4f77bf62ec1d85af8900cfb1d356d18292b09f7288ba178aea57324d7c48bd814e61adf296089e2e5c4db2e262ab6ebefdd854d28803348773d33c2c91446a10ac1a531e944552d9b50b394e2a4aff68ac2b05318307e0c918142f7251cbf67c19614a236a190610b145cc821c146a055a116731c6e1999a9d6aee9548782a52e2c9e75b866373c09ec17e01900917bb29e1d220241e30ce471f9d48944b8e7626e738d53bd64468cadb71e0e27bfd03fc875e522b580b2a6e77b363d424cebb0fba52101c0d5dd7e8da4edcc8a220a1b520c82 \
  0x190c853d5e68ed726abfe2d7f53d15bd40f4e3c501c819bcadb1166f0f24dbdd \
  0x25bbee2d15ce2822c5feaf388b32ff980549de93783b59778613b2cf68abbe2d \
  1 \
  --rpc-url $RPC_URL --private-key $PRIVATE_KEY $GAS_ARGS > /dev/null
echo "✅ 投票処理完了"

echo "-----------------------------------------------------"

echo "Step 6: 最終開票結果の確認！"
VOTES=$(cast call $VOTING_ADDR "votes(uint256)(uint256)" 1 --rpc-url $RPC_URL)
echo ""
echo "====================================================="
echo " 候補者1の得票数: ${VOTES} 票"
echo "====================================================="
