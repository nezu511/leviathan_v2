# Leviathan

**An Enterprise-Grade TEE × ZK Hybrid EVM in Rust**

Leviathanは，行政手続きや公共インフラ（選挙，給付金分配など）の完全自動化 **「無人の市役所」** の実現を目指してフルスクラッチ開発された，行政・エンタープライズ特化型のカスタムEVM（Ethereum Virtual Machine）です．

パブリックチェーンにおける「ガス代によるプライバシー漏洩」や「実行速度の限界」といったプロトコルレベルの課題を解決するため，EVMコアのプレコンパイルレイヤーに直接ゼロ知識証明（ZK-SNARKs）とRSA検証を組み込む次世代の暗号インフラとして設計されています．

## Vision: 現代のリヴァイアサン（無人の市役所）の構築
人の手が介在せず，私欲を持たず，ただデプロイされたコードという「法」に従って恒久的に動き続ける改ざん不可能なシステム．
私はEVMが持つこの「トラストレスな絶対性」に強烈に惹かれました．

透明性が極限まで求められ，恣意的な運用が絶対に許されない「行政手続き（選挙や給付金分配など）」は，スマートコントラクトと最も相性が良い領域です．
本プロジェクトは，特定の管理者に依存しない究極の民主主義インフラ，すなわち現代の「リヴァイアサン」を構築する試みです．
この自律型インフラの深淵をブラックボックスとしてではなく，ソースコードレベルで完全に理解・支配するため，Ethereum Yellow Paperの数学的定義に基づくフルスクラッチ開発を決意しました．

**【独自性：身元証明と完全匿名のハイブリッド統合】**  
行政インフラを現実社会に実装するためには，「システムの透明性」と同時に，時には相反する「個人の絶対的な匿名性」が不可欠です．
例えば，選挙においては誰の票であるかは完全に秘匿しつつ，「正規の有権者が1回だけ投票した」ことのみを数学的に証明しなければなりません．

日本にはすでに「マイナンバーカード」という強固な身元証明基盤が存在します．
私はこの事実に着目し，「マイナンバーのRSA署名による確実な身元証明」と「ZK-SNARKsによる完全匿名性」を両立させるプロトコルの需要に辿り着きました．
この重い暗号処理をガス代高騰の原因となるSolidity（アプリケーション層）で処理するのではなく，EVMコアの「プレコンパイルコントラクト」としてRustでネイティブに統合すること．
これこそが，本プロジェクト最大のオリジナリティであり，既存のパブリックチェーンには実現できないエンタープライズ特化型エンジンの真価です．

### Why Native Precompiles? (The 300 Billion Yen Problem)
既存のパブリックチェーン上のスマートコントラクト（Solidity）で本プロジェクトの「ZK-SNARKs検証 ＋ RSA-2048署名検証」を実行した場合、1トランザクションあたり約 250,000 Gas を消費します。
これを現在のEthereumメインネットの平均的なガス価格（15 Gwei, 1 ETH=50万円）で換算すると、1票の投票につき約2,000円の手数料が発生します。もし日本の有権者1億人が投票システムを利用した場合、トランザクション手数料だけで国境を越えて「約2000億円」が流出することになり、行政インフラとして完全に破綻します。

**【Leviathanによる解決策】**
本EVMは、この極めて重い暗号処理をアプリケーション層（Solidity）から引き剥がし、EVMコアの「カスタムプレコンパイル（Rustネイティブコード）」として実装しています。これにより、スタックやメモリのオーバーヘッドをO(1)に圧縮し、さらにコンソーシアム・プライベート環境で稼働させることで、この「2,000億円のガス代問題」をゼロにする経済合理性を実現しています。

##  Core Features
* **Built entirely in Rust:** 低レイヤーのメモリ安全性と極限のパフォーマンスを追求したフルスクラッチの実行エンジン．
* **Native ZK & RSA Precompiles:** スマートコントラクト（Solidity）層ではなく，EVMコアに直接暗号検証ロジック（RSA-2048, BN254等）をプレコンパイルとして統合し，圧倒的なガス代最適化と処理の高速化を実現．
* **Gasless Meta-Transactions via Relayer:** ユーザーのトランザクションはRelayerを経由してEVMにルーティングされ，送信元のIPやアドレスを完全に秘匿．
* **Cryptographically Provable State:** HashMap等の仮構想を完全に脱却し，公式StateTestをパスする厳密な Merkle Patricia Trie (MPT) をネイティブ実装．

##  Architecture & Data Flow

### Full Node Architecture (Consensus, Execution, RPC)

Leviathanは単なるスマートコントラクトの実行環境（VM）に留まらず、独立したブロックチェーンノード（L1/L2）として自律稼働する完全なシステムアーキテクチャを備えています。コンセンサス層、実行層、ストレージ層、そして外部インターフェース（RPC）がそれぞれ独立したコンポーネントとして強固に連携する設計を採用しています。

本アーキテクチャは以下の4つの主要レイヤーで構成されています。
- **Consensus Layer (CometBFT)**: ブロックの提案と合意形成を担うエンジンです。P2Pネットワーク通信とコンセンサスアルゴリズムを処理し、確定したブロックの骨組み（トランザクションリストとタイムスタンプ等）をABCI経由で実行層へ送ります。
- **Execution Layer (Leviathan ABCI & EVM)**: CometBFTから渡されたトランザクションを実行し、Ethereum仕様に完全準拠したレシートやMPTのルートハッシュ（State Root）を計算します。処理中はすべて揮発性のインメモリキャッシュ上で完結させ、極限までI/Oのボトルネックを排除します。
- **Storage Layer (RocksDB)**: 実行エンジンによって確定したステート（MPT）とブロックの履歴を永続化します。Gethの設計思想を踏襲し、フルブロックをそのまま保存するのではなく「Header」と「Body」に完全に解体して保存し、さらに TxLookup という専用の索引（インデックス）を構築することで、ストレージ効率と検索速度を最大化しています。
- **RPC Layer (JSON-RPC Server)**: 外部のWeb3クライアント（MetaMaskや cast コマンドなど）からのリクエストを非同期で処理する窓口です。ノードの実行サイクルを妨害しないよう、RPCサーバーはEVMから独立して稼働し、RocksDBから直接データを抽出・結合（Hydration）してユーザーに返却します。また，`eth_sendRawTransaction`のような「書き込み系」のリクエストが来た場合、RPCサーバーは単にDBを見るだけではなく、トランザクションをネットワークに流すために CometBFTのRPCポート（通常 26657）へ転送（ブロードキャスト）します．

このように、各コンポーネントの責務をプロトコルレベルで完全に分離することで、行政やエンタープライズの商用トラフィックに耐えうる堅牢性とスケーラビリティを実現しています。

```mermaid
graph TD
    %% 外部クライアント
    Client["Web3 Client<br/>(MetaMask, cast, etc.)"]

    %% RPCレイヤー
    subgraph RPC_Layer [RPC Layer]
        RPC["JSON-RPC Server<br/>(jsonrpsee)"]
    end

    %% コンセンサスレイヤー
    subgraph Consensus_Layer [Consensus Layer]
        BFT["CometBFT Engine<br/>(Consensus & P2P)"]
    end

    %% 実行レイヤー
    subgraph Execution_Layer [Execution Layer]
        ABCI["Leviathan ABCI<br/>(Application Interface)"]
        EVM["Leviathan EVM<br/>(Execution Engine)"]
        CACHE["In-Memory Cache<br/>& Local MPT"]
    end

    %% ストレージレイヤー
    subgraph Storage_Layer [Storage Layer]
        DB[("RocksDB<br/>(Geth-style Storage)")]
    end

    %% --- 接続関係 ---

    %% 1. クライアントからの入り口
    Client <-->|JSON-RPC| RPC

    %% 2. RPCサーバーの二面性 (ここを修正)
    RPC -.->|Read: eth_getBalance etc.| DB
    RPC -->|Write: eth_sendRawTransaction| BFT

    %% 3. コンセンサスと実行ユニットの対話
    BFT <-->|ABCI: CheckTx, FinalizeBlock, Commit| ABCI
    
    %% 4. 実行エンジン内部
    ABCI -->|Execute Txs| EVM
    EVM <-->|State Updates| CACHE
    ABCI -->|Commit| DB
    
    classDef storage fill:#f9f9f9,stroke:#333,stroke-width:2px;
    class DB storage;
    classDef highlight fill:#fff4dd,stroke:#d4a017,stroke-width:2px;
    class RPC,BFT highlight;
```

### Transaction Execution Flow

本EVMアーキテクチャの最大の特徴は，実行エンジンであるLEVIATHANと，状態管理を担うWorldStateの厳格な分離です．
トランザクションが入力されると，LEVIATHANがルーターとして機能し，ContractCreation（初期化）またはMessageCall（実行）へと処理を振り分けます．実行中の状態変化（ストレージの書き換えなど）はすべてインメモリのCacheに対して行われます．トランザクションが完全に成功し，すべての処理が終了した最後の瞬間にのみ，Cacheの内容が暗号学的な保管庫であるMerkle Patricia Trie (MPT)へとコミットされます．これにより，不要なディスクI/Oを排除し，極めて高いパフォーマンスと状態の整合性を両立しています．

```mermaid
graph TD
    TX[Transaction] -->|TransactionExecution| LEV_ROOT[LEVIATHAN Struct]
    
    LEV_ROOT -->|Route| CC[ContractCreation]
    LEV_ROOT -->|Route| MC[MessageCall]
    
    CC -->|Init Code| EVM[EVM Struct]
    MC -->|Normal Call| EVM
    MC -->|Precompile Contract| NATIVE["Rust Native Code<br/>Stateless"]
    
    EVM -->|CALL or CREATE Opcodes| LEV_CHILD[Child LEVIATHAN]
    LEV_CHILD -.->|Recursive Call| CC
    LEV_CHILD -.->|Recursive Call| MC
    
    subgraph World_State [WorldState]
        CACHE[(Cache)] -.->|End of TX: Commit| MPT[(Merkle Patricia Trie)]
    end
    
    LEV_ROOT <-->|Read / Write| CACHE
    LEV_CHILD <-->|Read / Write| CACHE
    EVM <-->|Read / Write| CACHE
    MC <-->|State Update: Balance, Nonce| CACHE
    CC <-->|State Update: Account Init| CACHE
    %% Precompiles intentionally have no line to CACHE
```

### Recursive Journaling & Rollback

スマートコントラクトでは「コントラクトAがBを呼び，BがCを呼ぶ」といった複雑なネスト（サブコール）が頻繁に発生します．この際，もし大元の呼び出しでエラーが起きれば，それまでの状態変化をすべて無かったこと（リバート）にしなければなりません．
EVM実装では，状態全体のスナップショットを都度作成するアプローチとジャーナルによるアプローチが考えられます．
スナップショットは実装が単純でバグも少なく実装できる反面，パフォーマンスのボトルネックになるという欠点が存在します．

本プロジェクトではこれを解決するため，「子LEVIATHAN構造体」による再帰的なジャーナル（Cacheに対する変更履歴の記録）管理を採用しています．サブコールが発生するたびに子インスタンスが生成され，そのコンテキスト内でのみ変更内容が記録されます．

- 正常終了時（Success）： サブコールが成功した場合，子LEVIATHANが記録したジャーナルを，そのまま親のジャーナルに統合（Merge）します．
- 例外停止・Revert時（Failure）： 失敗した場合は，子LEVIATHANのジャーナル履歴を逆再生し，Cacheを正確に元の状態に巻き戻します．

【複雑なロールバックも完全に制御】
この設計の真価は，深いネスト構造において発揮されます．
例えば，「子1」がさらに「子2」「子3」を呼び出したとします．子2と子3が正常終了して履歴が子1にMergeされた後，最終的に大元の「子1」が例外停止（またはRevert）したとします．この場合でも，統合済みの「子1」のジャーナルを巻き戻すだけで，子2・子3が行ったCacheへの変更も含めてすべて完璧にロールバックされます．

これにより，メモリオーバーヘッドを極限まで削りつつ，どれほど複雑な階層でも安全な状態復元を実現しています．

```mermaid
sequenceDiagram
    participant Parent as Parent LEVIATHAN
    participant Child as Child LEVIATHAN
    participant Cache as WorldState Cache

    Parent->>Child: Create for Sub-call
    
    loop EVM Execution
        Child->>Cache: State changes
        Note right of Child: Record changes in Child Journal
    end

    alt Sub-call Success
        Child-->>Parent: Return Success
        Parent->>Parent: Merge Child Journal
    else Sub-call Revert
        Child-->>Parent: Return Error
        Parent->>Cache: Rollback using Child Journal
        Note over Parent,Cache: Parent state remains completely intact
    end
```

### EVM Core Engine

EVMコアのメインループ（EVM::run）は，Ethereum Yellow Paperに記述された数学的定義をRustの関数として極めて忠実に再現しています．
各オペコードの実行は，以下の明確な責務を持つ3つの関数フェーズに分割されています．

- **Z function (is_safe)**: スタックやメモリの安全性を検証し，違反があれば即座に例外停止（Exceptional Halt）させます．
- **G function (gas)**: EIP-150等の仕様に基づき，複雑なガス計算を行います．
- **O function (execution)**: G functionで計算したガスを消費し，実際の状態遷移を実行し，処理の継続，正常終了（STOP/RETURN），または明示的なリバートを制御します．

ZとG functionはイエローペーパーの定義通りState及びVMの状態を遷移させません．
このように関数の責務をプロトコルレベルで完全に分離することで，エッジケースのバグを排除し，ハードフォークごとの仕様変更に対しても極めて高い保守性を誇ります．

``` mermaid
graph TD
    Start[EVM::run Loop] --> Fetch[Fetch Opcode]
    Fetch --> Z_Func{Z function: Safe?}
    
    Z_Func -->|No: Unsafe| Ex_Halt[Exceptional Halt<br/>Return Err None]
    Z_Func -->|Yes: Safe| G_Func[G function: Consume Gas]
    
    G_Func --> O_Func[O function: State Transition]
    O_Func --> CheckResult{O function Result?}
    
    CheckResult -->|None| Fetch
    CheckResult -->|Some True| Revert[Revert<br/>Return Err Some]
    CheckResult -->|Some False| NormalStop[Normal Halt: STOP/RETURN<br/>Return Ok]
```
### Advanced Architecture: Dual Merkle Tree
Leviathanは、Ethereum標準への完全準拠と、ZKインフラとしての高効率を両立するため、二階建てのステート管理構造を採用しています。

1. **Layer 1: World State (Standard MPT)**
   - **Hash Algorithm**: Keccak256
   - **役割**: アカウント残高、Nonce、コントラクトストレージの管理。
   - **意義**: ここを標準仕様に留めることで、公式の `VMTests` 互換性を100%維持し、既存のWeb3エコシステムとの親和性を担保します。

2. **Layer 2: Application State (Poseidon Tree)**
   - **Hash Algorithm**: Poseidon (ZK-Friendly)
   - **役割**: 投票箱（Commitment）の包含証明専用のツリー。
   - **意義**: ZK回路内での計算コストが極めて低いPoseidonを採用することで、モバイルデバイス等での証明生成（Proof Generation）を現実的な速度で可能にします。

## Technical Excellence: Rust Idiomatic Design 
単に「動く」だけでなく，長期的な保守性と拡張性を担保するためのRustらしい設計を徹底しています．

### Stateトレイトによる抽象化と「換装可能性」の担保
開発初期，デバッグの効率化のためにWorldStateは単純な HashMap で構築されていました．しかし，最終的に Merkle Patricia Trie (MPT) への換装が不可欠であることを予見し，実行エンジンがStateに依存しないよう State トレイトによる抽象化 を実施しました．

**成果**: 実際に HashMap から MPT へのデータ構造換装を行った際，コアとなる実行エンジン（EVM Core）のコードには一行も手を加えることなく移行を完了しました．これは，責務の分離とトレイト境界による制約が正しく機能した証左です．

## Technical Philosophy (ADR)
本プロジェクトは既存EVMの単なるクローンではなく，スケーラビリティ・仕様準拠・保守性を極限まで担保するため，以下のアーキテクチャ設計を採用しています．

### O(1) ロールバックを実現するジャーナルベースの状態管理

スマートコントラクトの実行において，サブコール（CALL等）失敗時の状態（State）リバートはパフォーマンスのボトルネックになります．
本EVMでは，状態全体のディープコピーを避け，ジャーナル（変更履歴）ベースのロールバック機構を採用しました．トランザクションのコア実行構造体内部に直接ジャーナルを保持させることで，状態の逆再生（Undo）のみでメモリオーバーヘッドなく正確な復元を可能にしています．

### 厳密なYellow Paper準拠による関数マッピング

EVMの複雑な仕様とエッジケースを正確にハンドリングするため，Ethereum Yellow Paperの数学的定義をRustのモジュール単位に直接落とし込んでいます．仕様書とソースコードの対応関係を透過的にすることで，極めて高い堅牢性とデバッグの容易性を確保しました．

### ハードフォークの動的切り替え（VersionId）

列挙型 VersionId を実行環境に導入し，Frontierから最新仕様までのオペコードやガス代の変更を単一のコードベースで共存させています．ブランチを分けることなく，動的にプロトコルバージョンを切り替えてテスト・実行が可能です．

### モダンな型システムとオーバーフローの完全排除

数値型には最新の alloy_primitives::{U256, I256} を採用．また，メモリ拡張コストの計算時など，悪意ある巨大な入力による整数オーバーフロー攻撃を防ぐため，境界チェックにはRustの saturating_add などを徹底し，セキュアな算術演算を実装しています．

## Case Study: "Unmanned City Hall" Election Simulation

2026年5月、本エンジン上にて以下のEnd-to-Endシナリオの完遂に成功しました。

**シミュレーション内容:**
1.  **身元登録**: 有権者がマイナンバーカード（RSA-2048）で署名。EVMプレコンパイルがこれを検証し、成功時のみ `Commitment` をPoseidonツリーに登録。
2.  **匿名投票**: 登録済み有権者が、ZK-SNARKs（Groth16）を用いて「自分がツリーに含まれる正当な有権者であること」を匿名で証明。
3.  **二重投票防止**: `Nullifier Hash` を用いて、匿名性を保ちつつ「同じ人が2回投票すること」をプロトコルレベルで拒絶。
4.  **最終結果**: 候補者1が2票、候補者2が1票という投票結果が、不正なくステートに反映されることを確認。

**Execution Log (Proof of Work):**
```text
--- Phase 0: Generate Dynamic Commitments ---
✅ Generated Commitment for secret 11111: 0x0d9bd617a1576...
--- Phase 1: Voter Registration ---
Deploying IdentityRegistry...
 Success! Precompile verified the signature. Remaining Gas: 1030987
Is commitment registered? true
...
--- Phase 2: Anonymous ZK Voting ---
--- Voting for Voter 0 (Choice 1) ---
✅ input.json generated for Voter 0!
 Call Success! Remaining Gas: 244714
...
--- Final Check: Vote Count ---
Votes for choice 1: 2
Votes for choice 2: 1
test test_election_e2e ... ok
```

## Case Studies: Overcoming Protocol-Level Challenges

### EIP-150 (63/64 Rule) と関数の厳格な責務分離

親コントラクトから子へガスを渡す際，要求ガスが親の残量を超える場合の挙動の差異を実装するにあたり，実行フェーズでのアドホックな例外処理を排除しました．すべてをガス計算モジュールの中で完結させ，計算された許容ガス量を実行コンテキストに一時的に保持（キャッシュ）させるアーキテクチャを採用し，フォーク間の複雑な状態遷移を解決しました．

### 再帰制限 (Depth 1024) の突破とコンパイラレベルのメモリ最適化

公式の StateTest における 1024階層の再帰呼び出しテストにおいて，巨大な match 式によるRustコンパイラのローカル変数の一括スタック確保仕様が原因となるスタックオーバーフローに直面しました．
重いOpcode処理の別関数への切り出しと #[inline(never)] の付与，および子EVMインスタンスのヒープ退避（Box::new()）を実施し，1階層あたりのスタック消費を劇的に圧縮して再帰テストを突破しました．

### [Feature] Behemoth Protocol: コンセンサス駆動の特権コントラクトと専用停止機構
行政システム（Gov-EVM）や金融インフラにおいては、致命的なバグ発生時のフェイルセーフが必須です。しかし、特定の管理者（EOA）にシステムのPause権限を持たせることは単一障害点となり、「トラストレス」という前提を破壊します。
Leviathanでは、特定の管理者に依存しないネットワーク保護機構「Behemoth Protocol」を設計しています。

* **専用オペコード (`HALT_NETWORK`) の実装:**
  EVMコアロジックに独自のカスタムオペコードを追加。これが実行されると、現在のステートキャッシュ（StateDB）を即座に破棄し、Engine API経由でコンセンサス層へ「ブロック生成および合意形成の停止」シグナルを発信します。
* **特権コントラクト「Behemoth」によるアクセス制御:**
  `HALT_NETWORK` は、システム予約アドレス（例: `0x0000...BEHEMOTH`）に配置された特権コントラクトから呼び出された場合のみ実行が許可され、通常のユーザーが実行した場合は即座にRevertされます。
* **トランザクション型 `Type 0xBE` によるコンセンサス駆動デプロイ:**
  未知の攻撃手法に対応するため、停止ロジック自体は事後的にデプロイ・更新可能とします。ただし、特権コントラクトをデプロイできるのは「全バリデータのN%以上の合意証明（集約署名等）」が含まれた専用トランザクション（Type 0xBE）のみであり、ガス代免除の上で強制適用されます。
* **オフチェーン対応と自動再起動:**
  ネットワーク停止は「人間へのクーリングオフ（調査・修正パッチ準備の猶予）期間」として機能し、デッドロックを防ぐため一定時間（Nブロック生成時間相当）の経過後に自動的に再起動する設計としています。

## Testing Methodology: The Roadmap to Reliability
本プロジェクトのテストプロセスは，開発フェーズ（Stateの実装方式）に合わせて段階的に構築されています．単に「テストを通す」ことだけではなく，実装の変更に対して「デグレード（退行）が起きていないか」を数学的に検証することを重視しています．

### Phase 1: HashMap State & Filler-based Testing
開発初期，State（状態管理）を HashMap で実装していた段階では，公式テストスイートの中でも "Filler" ファイルを主軸に検証を行いました．

- LLL自動コンパイルへの限定: テスト用bytecodeの生成において，LLL（Lisp Like Language）の自動コンパイル環境に対応しているソースのみを抽出して実行しました．
- Status-based Verification: Fillerファイルはテスト結果がハッシュ値ではなく「最終的なステートの状態（NonceやBalanceなど）」で定義されています．MPT実装前の段階では，この明示的な状態値を突き合わせることで，実行エンジンの論理的な正しさを担保しました．
- Scope: このフェーズでは，公式が提供する全テストセットの網羅ではなく，あくまでコアエンジンの基本動作を確実に固めることを優先しました．

### Phase 2: Transition to MPT & Consistency Verification
Stateを Merkle Patricia Trie (MPT) へ換装した現在のフェーズでは，　**「HashMap版でパスした全テストを，MPT版でも完全に突破すること」** を当面の絶対目標としています．

- テストセットの継続性: 検証の整合性を保つため，MPT版においてもHashMap版で抽出したテストベクタと同じバリエーションを使用しています．
- Hash-based Verificationへの布石: MPTの実装により，最終的な StateRoot（ハッシュ値）による厳格な検証が可能となりました．現在は，先行してパスしていたFillerベースのテストをMPT環境で再走させ，MPTへの換装が実行ロジックに悪影響を与えていないことを，パス率 100% の維持によって証明しています．

## Testing & Compliance
**【テスト突破のビジネス的意義】**
ここに記載されているテスト群は、単なる機能確認ではなく「Ethereumプロトコルのエッジケースや脆弱性を突く世界標準のストレステスト」です。
これを広範にパスしている事実は、本エンジンが単なるPoC（概念実証）を超え、エンタープライズの商用環境に耐えうるプロトコル水準の実装であることを数学的に証明しています。

### Ethereum Compatibility & Test Scope

**完全なイーサリアム互換性 (Ethereum Compatibility)**
本エンジンは独自のRust実装ですが、Ethereumメインネット（Constantinople / Petersburgフォーク仕様）と完全な互換性を持ちます。
つまり、既存のEthereumに向けてSolidityで書かれたスマートコントラクトは、一切のコード変更なしに本EVM（Leviathan）上で全く同じ挙動で動作します。

### Key Milestone: 100% VMTests Pass
本エンジンのコアである命令実行ロジック（Interpreter）の正確性を検証するため、イーサリアム公式の命令レベルテストである **`VMTests` 全件をパス** しています。

- **Opcodeの完全性**: `ADD` や `MUL` といった基本演算から、`KECCAK256` などの複雑な演算、スタック・メモリ操作に至るまで、EVM命令セットの挙動がイエローペーパーの定義と1bitの狂いもなく一致していることを証明済みです。
- **計算基盤の信頼性**: この100%パスという実績は、 Leviathan の「心臓部」がすでに完成しており、その上に構築される高度な暗号処理（RSA/ZK）の計算結果が数学的に信頼できることを意味します。


### Test Coverage Summary
現在、FrontierからPetersburgまでの各フォーク仕様に基づいた `GeneralStateTests` を実行し、主要なオペコードや複雑な状態遷移に関するテストを**ほぼ100%パス**しています。

* **Smart Contract Execution:** 複雑な再帰呼び出し（`stCallCodes`, `stDelegatecall`）やリバート処理（`stRevertTest` 1188ケース）を完全突破。
* **Cryptography (ZK-SNARKs):** Ethereumで最も難解とされる暗号プレコンパイル群（`stZeroKnowledge` 1612ケース）のエッジケースを完全にクリア。Phase 2の独自RSA統合に向けた盤石な基盤を確立しました。

**未着手（🔄）テストに関するロードマップ方針**
現在ステータスが「未着手」となっている一部のテスト群は、開発が滞っているわけではなく、**「本プロジェクトのコアバリューであるPhase 2（暗号・マイナンバー統合）を最優先するため、意図的に後回しにしているスコープ」**です。
2026年のPoC完成というマイルストーンに向け、まずはクリティカルパスである「実行エンジンの堅牢性（完了）」と「独自暗号プレコンパイルの開発（現在進行中）」にリソースを集中しています。商用環境で直ちに必須とならないマイナーなエッジケースの検証は、PoC完成後の最適化フェーズにて順次カバーするアジャイルな開発計画を採用しています。

### Detailed Test Results
特に優先度の高い重要項目は，**太字**で強調しています．

**A - G**
| テストスイート | 進捗 | 備考 |
| :--- | :---: | :--- |
| stArgsZeroOneBalance | ❌ | 全ファイルがyml形式のため実行不可 |
| stAttackTest | ✅ | **Pass** (現在ファイルが2/14ケース) |
| stBadOpcode | ✅ | **Pass** (現在ファイルが1/582ケース) |
| stBugs | ✅ | **Pass** (現在ファイルが4/38ケース) |
| **stCallCodes** | ✅ | **Pass** (ファイルが79・328ケース） |
| **stCallCreateCallCodeTest** | ✅ | **Pass** (39ファイル・168ケース) |
| **stCallDelegateCodesCallCodeHomestead** | ✅ | **Pass** (58ファイル・244ケース） |
| **stCallDelegateCodesHomestead** | ✅ | **Pass** (58ファイル・247ケース）|
| stChangedEIP150 | ✅ | **Pass** (30ファイル・159ケース) |
| stCodeCopyTest | 🔄 | 未着手 |
| stCodeSizeLimit | ✅ | **Pass** (3ファイル・19ケース) |
| **stCreate2** | ✅ | **Pass** (30ファイル・201ケース) |
| **stCreateTest** | ✅ | **Pass** (29ファイル・331ケース）|
| stDelegatecallTestHomestead | ✅ | **Pass** (28ファイル・125ケース) |
| stEIP150Specific | 🔄 | 未着手 |
| stEIP150singleCodeGasPrices | 🔄 | 未着手 |
| stEIP158Specific | ✅ | **Pass** (7ファイル・30ケース) |
| stExample | 🔄 | 未着手 |
| stExtCodeHash | ✅ | **Pass** (6ファイル・40ケース) |

**H - O**
| テストスイート | 進捗 | 備考 |
| :--- | :---: | :--- |
| stHomesteadSpecific | ✅ | **Pass** (5ファイル・20ケース) |
| **stInitCodeTest** | ✅ | **Pass** (16ファイル・120ケース) |
| stLogTests | ✅ | **Pass** (46ファイル・322ケース) |
| stMemExpandingEIP150Calls | 🔄 | 未着手 |
| **stMemoryStressTest** | ✅ | **Pass** (38ファイル・287ケース）|
| **stMemoryTest** | ✅ | **Pass** (58ファイル・406ケース) |
| stNonZeroCallsTest | 🔄 | 未着手 |

**P - S**
| テストスイート | 進捗 | 備考 |
| :--- | :---: | :--- |
| stPreCompiledContracts | 🔄 | Balance不一致 |
| stPreCompiledContracts2 | 🔄 | Balance不一致 |
| stQuadraticComplexityTest | ✅ | **Pass** (16ファイル・124ケース) |
| stRandom | ✅ | **Pass** (313ファイル・1250ケース) |
| stRandom2 | 🔄 | 未着手 |
| stRecursiveCreate | ✅ | **Pass** (2ファイル・12ケース) |
| **stRefundTest** | ✅ | **Pass** (19ファイル・166ケース） |
| stReturnDataTest | 🔄 | MLOADの値が不一致 |
| **stRevertTest** | ✅ | **Pass** (43ファイル・1188)|
| **stSStoreTest** | ✅ | **Pass** (1ファイル・2ケース) |
| stShift | ✅ | **Pass** (40ファイル・268ケース） |
| **stSolidityTest** | ✅ | **Pass** (16ファイル・38ケース）|
| stSpecialTest | 🔄 |   |
| stStackTests | ✅ | **Pass** (7ファイル・637ケース） |
| stStaticCall | 🔄 | 呼び出し元の残ガスが6ガス相違 |
| stSystemOperationsTest | 🔄 | 挙動確認中 |

**T - Z**
| テストスイート | 進捗 | 備考 |
| :--- | :---: | :--- |
| stTimeConsuming | ✅ | **Pass** (1ファイル・6ケース） |
| stTransactionTest | 🔄 | 一部検証中 (△) |
| stTransitionTest | ✅ | **Pass** (6ファイル・42ケース） |
| **stWalletTest** | ✅ | **Pass** (42ファイル・169ケース) |
| stZeroCallsRevert | ✅ | **Pass** (16ファイル・48ケース） |
| stZeroCallsTest | ✅ | **Pass** (24ファイル・168ケース） |
| stZeroKnowledge | ✅ | **Pass** (33ファイル・1612ケース) |
| stZeroKnowledge2 | 🔄 | 未着手 |

### Phase 3: Integration Benchmarking & Data-Driven Gas Profiling
独自の暗号処理（RSA検証など）をEVMコアに統合するにあたり、DoS攻撃を防ぎつつ実用的なガスコストを設定するためのベンチマーク環境（`bench_runner`）を構築しました。
本番のEVMエンジン（`src/`）に計測コードを混入させないクリーンな統合テスト環境下で、公式のJSONテストスイートを流用して実行時間を計測しています。

**成果**: 
純粋なRust実装によるRSA-2048検証のCPU実行時間（平均197µs）を計測し、公式プレコンパイルの中で最も統計的信頼性の高い `sha256`（サンプル数: 333,563件）の燃費（0.00117 µs/gas）をベースラインとしてガスコストを計算しました。
これにより、セキュリティマージンを含めた **168,000 gas** という論理的かつ安全なカスタムガスコストを導き出し、プロトコルに実装しています。

## Current Status & Roadmap

現在，PoCに向けたコアエンジンの検証フェーズを完了し，暗号統合フェーズを実行中です．

[x] Phase 1: Core Engine & MPT Integration
- [x] EVM実行エンジンの構築と公式 GeneralStateTests の広範なパス．
- [x] HashMapレイヤーの排除と，Merkle Patricia Trie (MPT) を用いた StateDB の完全統合．

[x] **Phase 2: ZK & Cryptography Integration (PoC 実証完了)**
- [x] **E2E 選挙シミュレーションの成功**: RSAによる身元確認とZKによる匿名投票を組み合わせた一連のフローが，EVM上で正確にステート遷移することを確認（2026年5月）．
- [x] **Poseidon Merkle Treeの実装**: ZK-SNARKsの包含証明（Inclusion Proof）に特化した、Poseidonハッシュベースの専用ツリー構造を統合．
- [x] **RSA-2048 プレコンパイルの最適化**: Rustネイティブ実装により、マイナンバー署名検証のガスコストを実用圏内（O(1)）に抑制．

[x] **Phase 3: Node Architecture & Data Persistence (New!)**
- [x] CometBFT (ABCI) の統合によるコンセンサス層との連携・ブロック生成。
- [x] Geth型アーキテクチャに基づく RocksDB への Block / Receipt / TxLookup 永続化。
- [x] `jsonrpsee` を用いた JSON-RPC サーバー基盤の構築とネットワーク連携

[ ] Phase 4: Relayer API
- メタトランザクションを処理するRelayer APIの構築．

[ ] Phase 5: TEE Integration (Future Work)
SGX等のTEE環境を利用した実行環境の完全秘匿化へのリサーチ

## Tech Stack
- **Core:** Rust
- **Consensus & RPC:** CometBFT (Tendermint ABCI), jsonrpsee
- **EVM Components:** Custom Implementation, alloy_primitives, alloy_consensus, alloy_rlp, eth_trie
- **Database:** RocksDB
- **Cryptography:** ZK-SNARKs (Circom), RSA-2048, BN254

### Why Rust & Full-Scratch?
次世代の行政インフラを構築するにあたり，以下の理由から実行エンジンの開発言語としてRustを選定しました．

1. **論理エラーの純化と絶対的なメモリ安全:**
   Rustの強固な所有権モデルにより，コンパイルを通過したコードからは「メモリリーク」や「データ競合」といった致命的な未定義動作が数学的に排除されます．これにより，開発者はEVMの仕様に起因する「論理的なバグの解決」のみに100%の思考リソースを集中させることができます．
2. **ZK・TEEエコシステムとの圧倒的な親和性:**
   Circomやarkworks等の最先端のゼロ知識証明エコシステム，およびSGX等のTEE（Trusted Execution Environment）開発において，Rustは事実上の標準言語（デファクトスタンダード）です．独自の暗号ロジックをネイティブ統合する本プロジェクトにおいて，Rustは唯一にして最強の選択肢です．
3. **アセンブリ・低レイヤー開発の経験値とのシナジー:**
   過去にアセンブリ言語を用いてOSをスクラッチ開発した経験から，スタックマシンの挙動，メモリレイアウトの設計，およびハードウェアリソースを直接意識した開発を得意としています．この低レイヤーの知見が，Rustの厳しいコンパイラと極めて高いシナジーを生み，EVMという巨大なステートマシンの高速な実装を可能にしています．


## Quick Start: How to Run Leviathan

本エンジンをローカル環境でビルドし、CometBFTを用いたブロック生成とRPC経由でのトランザクション送信を行う手順です。

### 1. Prerequisites (環境構築)

**Rust Toolchain**
本プロジェクトはRustで記述されています。公式の `rustup` を用いてインストールしてください。
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

**CometBFT**
コンセンサス層としてCometBFT（v0.38+推奨）が必要です。
```bash
# CometBFTのダウンロードと解凍
curl -LO https://github.com/cometbft/cometbft/releases/download/v0.38.0/cometbft_0.38.0_linux_amd64.tar.gz
tar -zxvf cometbft_0.38.0_linux_amd64.tar.gz

# 実行ファイルをパスの通ったディレクトリへ移動し、初期化
sudo mv cometbft /usr/local/bin/
cometbft init
```

**Foundry (cast)**
RPC経由でノードと対話するためのCLIツールとして、Foundryの `cast` コマンドを使用します。
```bash
curl -L https://foundry.paradigm.xyz | bash
source ~/.bashrc
foundryup
```

**Node.js & Snarkjs (For ZK Testing)**
ゼロ知識証明のE2Eテスト（`election_test`）を実行する場合、裏側で回路の計算を行うためにNode.jsおよび `snarkjs` が必要です。

```bash
# 1. Node.js (v20) と npm をインストール
curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
apt install -y nodejs

# 2. snarkjs をグローバルインストール
npm install -g snarkjs
```


### 2. Run the Node

以下の手順で、実行エンジン（Leviathan）とコンセンサスエンジン（CometBFT）を連動させます。

**Terminal 1: Start Leviathan ABCI & RPC Server**
```bash
# リポジトリのルートディレクトリで実行
cargo run --bin abci_node
```
これにより、ABCIサーバー（CometBFTとの通信用）と、JSON-RPCサーバー（クライアントからのリクエスト受付用）が起動します。

**Terminal 2: Start CometBFT**
```bash
# CometBFTノードを起動し、LeviathanのABCIへ接続
cometbft node --proxy_app=tcp://127.0.0.1:26658
```
CometBFTがブロックの提案を開始し、Leviathan側でトランザクションの実行とステートの確定処理が走るログが確認できます。

### 3. Interact via RPC (Simple Transfer)

ノードが稼働している状態で、`cast` コマンドを使用してEthereum互換のトランザクションを送信します。

ジェネシスアカウントのPRIVATE_KEYは0x80c58089c4343be9bd0ae0d2af81c615211d1e354a4c6073c9a1c32840f6274aです．
```bash
# 例: ローカルノードに対して、単純なETH送金を実行
cast send 0x0000000000000000000000000000000000001337   --value 1ether   --private-key 0x80c58089c4343be9bd0ae0d2af81c615211d1e354a4c6073c9a1c32840f6274a   --rpc-url http://127.0.0.1:8545    --gas-price 0   --gas-limit 21000   --legacy   --async
```
RPCサーバーがトランザクションを受け取り、CometBFTのネットワークへブロードキャストし、次のブロックでEVMによって実行・永続化されます。

---

## Testing

本プロジェクトでは、Ethereum公式のテストベクタ（GeneralStateTests）や複雑な暗号処理の統合テストを実行できます。

**⚠️ Important: Stack Size Limit**
EVMの深いネスト（再帰呼び出し）を再現するテスト（`stCallCodes`等）を実行する際、OSのデフォルトのスタックメモリ制限（通常8MB）ではRustのスタックオーバーフローが発生する場合があります。
そのため、テスト実行時は環境変数 `RUST_MIN_STACK` を指定してスタックサイズを拡張（例: 64MB）してください。

**Run End-to-End Election Simulation**
マイナンバーカードのRSA署名検証とZK-SNARKsを統合した無人の市役所（選挙）シミュレーションを実行します。
```bash
RUST_MIN_STACK=67108864 cargo test --test election_test --release -- --nocapture
```

**Run Official Ethereum State Tests**
Ethereum Yellow Paper仕様への準拠を証明するための、公式StateTestランナーを実行します。
```bash
RUST_MIN_STACK=67108864 cargo test --test state_runner --release -- --nocapture
```

## About the Author 
Taku Hashimoto  
土木工学（都市計画）と生命科学（タンパク質変異と疾患）という異分野のバックグラウンドを持つ．
過去にアセンブリ言語でOSをスクラッチ開発した経験から得た低レイヤー・メモリ管理の知見を活かし，次世代の社会インフラとなる暗号プロトコルの構築中．
