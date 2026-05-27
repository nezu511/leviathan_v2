#!/bin/bash

# エラーが発生した場合は即座にスクリプトを停止する
set -e

echo "====================================================="
echo " Leviathan: 無人市役所システム 4人同時 E2E テスト "
echo "====================================================="

# --- 基本設定 ---
export RPC_URL="http://127.0.0.1:8545"
export PRIVATE_KEY="0x80c58089c4343be9bd0ae0d2af81c615211d1e354a4c6073c9a1c32840f6274a"
export GAS_ARGS="--legacy --gas-limit 30000000 --gas-price 1"

# --- 4人の有権者データ ---
# Voter 1 ~ 4 の秘密情報と投票先（1=候補者A, 2=候補者B）
SECRETS=("11111" "22222" "33333" "44444")
NULLIFIERS=("10001" "20002" "30003" "40004")
CHOICES=("1" "1" "2" "1") # 候補者1に3票、候補者2に1票入る想定

echo "-----------------------------------------------------"
echo "Part 1: スマートコントラクトのデプロイ"
echo "-----------------------------------------------------"

echo "Step 1: IdentityRegistry のデプロイ..."
REGISTRY_ARGS=$(cast abi-encode "constructor(uint256)" 1000000 | sed 's/^0x//')
REGISTRY_BYTECODE=$(cat solidity/out/IdentityRegistry.bin | sed 's/^0x//' | tr -d '\n')
export IDENTITY_ADDR=$(cast send --rpc-url $RPC_URL --private-key $PRIVATE_KEY $GAS_ARGS \
  --create "0x${REGISTRY_BYTECODE}${REGISTRY_ARGS}" \
  | grep 'contractAddress' | awk '{print $2}')
echo "✅ IdentityRegistry デプロイ完了: ${IDENTITY_ADDR}"

echo "Step 2: VK_Data のデプロイ..."
VK_HEX=$(xxd -p -c 999999 solidity/out/VK_Data.bin | tr -d '\n')
export VK_ADDR=$(cast send --rpc-url $RPC_URL --private-key $PRIVATE_KEY $GAS_ARGS \
  --create "0x${VK_HEX}" \
  | grep 'contractAddress' | awk '{print $2}')
echo "✅ VK_Data デプロイ完了: ${VK_ADDR}"

echo "Step 3: Voting のデプロイ..."
ARGS=$(cast abi-encode "constructor(address,address)" $VK_ADDR $IDENTITY_ADDR | sed 's/^0x//')
BYTECODE=$(cat solidity/out/Voting.bin | sed 's/^0x//' | tr -d '\n')
export VOTING_ADDR=$(cast send --rpc-url $RPC_URL --private-key $PRIVATE_KEY $GAS_ARGS \
  --create "0x${BYTECODE}${ARGS}" \
  | grep 'contractAddress' | awk '{print $2}')
echo "✅ Voting デプロイ完了: ${VOTING_ADDR}"

# ⚠️ 注意: ここで一度CLI側のアドレスを書き換えて再ビルドする手順が必要ですが、
# 今回は環境変数をCLI側でも読み取れるように実装拡張している前提、
# もしくは固定アドレスで立ち上げている前提で進行します。

echo ""
echo "-----------------------------------------------------"
echo "Part 2: 4人の市民登録フェーズ (RSA署名検証)"
echo "-----------------------------------------------------"

for i in "${!SECRETS[@]}"; do
  echo "[Voter $((i+1))] 登録用ペイロードの生成と送信..."
  
  # CLIを実行し、結果を一時ファイルに出力
  ./target/release/voter_cli register --secret ${SECRETS[$i]} --nullifier ${NULLIFIERS[$i]} > temp_reg.txt
  
  # 出力されたテキストから "cast send" 以降の行（実際のコマンド）だけを抽出して実行スクリプト化
  sed -n '/cast send/,$p' temp_reg.txt > execute_reg.sh
  
  # 生成されたコマンドを実行
  bash execute_reg.sh > /dev/null
  echo "✅ Voter $((i+1)) 登録完了！"
done

echo ""
echo "-----------------------------------------------------"
echo "Part 3: 匿名投票フェーズ (ZK-SNARKs)"
echo "-----------------------------------------------------"

# 4人全員が登録し終わった後の、最新の公式ルートハッシュを取得
CURRENT_ROOT=$(cast call $IDENTITY_ADDR "currentRoot()(bytes32)" --rpc-url $RPC_URL)
echo "🌳 現在の公式Merkle Root: $CURRENT_ROOT"
echo ""

for i in "${!SECRETS[@]}"; do
  echo "[Voter $((i+1))] ZK Proofの生成と投票 (候補者: ${CHOICES[$i]})..."
  
  # 投票用のCLIを実行 (インデックスは配列のループ変数 i と完全に一致します)
  ./target/release/voter_cli vote \
    --secret ${SECRETS[$i]} \
    --nullifier ${NULLIFIERS[$i]} \
    --choice ${CHOICES[$i]} \
    --root $CURRENT_ROOT \
    --index $i > temp_vote.txt
    
  # "cast send" 以降のコマンド部分を抽出
  sed -n '/cast send/,$p' temp_vote.txt > execute_vote.sh
  
  # ZKPを含むトランザクションを送信
  bash execute_vote.sh > /dev/null
  echo "✅ Voter $((i+1)) 投票完了！"
done

# 一時ファイルのお掃除
rm temp_reg.txt execute_reg.sh temp_vote.txt execute_vote.sh

echo ""
echo "-----------------------------------------------------"
echo "Part 4: 運命の開票"
echo "-----------------------------------------------------"

VOTES_1=$(cast call $VOTING_ADDR "votes(uint256)(uint256)" 1 --rpc-url $RPC_URL)
VOTES_2=$(cast call $VOTING_ADDR "votes(uint256)(uint256)" 2 --rpc-url $RPC_URL)

echo ""
echo "====================================================="
echo " 🗳️ 開票結果 🗳️ "
echo " 候補者 1 : ${VOTES_1} 票"
echo " 候補者 2 : ${VOTES_2} 票"
echo "====================================================="

if [ "$VOTES_1" -eq 3 ] && [ "$VOTES_2" -eq 1 ]; then
    echo "🎉 テスト完全合格: 期待通りの得票数です！"
else
    echo "⚠️ 警告: 得票数が期待値と異なります。"
fi
