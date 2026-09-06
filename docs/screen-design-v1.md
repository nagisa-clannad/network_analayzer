# ネットワーク構成自動可視化・IPAMツール 画面設計 v1.5

- 対象: 統合仕様書 v0.4.11 の Phase 1
- 状態: 実装可能な画面・操作設計
- 基準: 「分からないものを、既知のもののように描かない」

## 1. 画面設計の原則

1. 構成図の線、機器、IP、判定結果は必ず `known` / `inferred` / `unknown` / `user_confirmed` を色以外でも区別する。
2. 利用者が画面上で見る自動判定には、Evidence Inspector へ一操作で到達できる根拠を付ける。
3. Scan、Scope、Credential、手動変更など書込み操作は、閲覧・検索・フィルタと視覚的に分離する。
4. 全体構成と詳細を同じ画面に詰め込まない。中心は Canvas、一覧、Inspector の三層とし、必要なときだけ詳細を開く。
5. 画面の選択、展開、レイヤー表示、検索は表示状態であり、Network Data を変更しない。変更は明示的な操作と Audit Trail を通す。

## 2. 情報アーキテクチャ

```text
Project switcher
├─ Overview
├─ Devices
├─ Topology
│  ├─ Physical
│  ├─ L2 Logical
│  ├─ L3 Logical
│  └─ Combined
├─ IPAM
├─ Changes
├─ Unresolved
├─ Scans
├─ Scope & Profiles
└─ Project settings
```

- 主ナビゲーションは左サイドバーに固定する。現在の Project と最後に完了した Scan status を常時表示する。
- Topology 内の View 切替は Workspace 内のタブに置き、別画面へ遷移させない。
- Evidence Inspector は右側の詳細パネルとして開く。狭い幅では同一領域の詳細画面に置換し、Canvas を隠す。
- Scan の開始は全 View 共通の上部アクションに置く。ただし必ず Preflight へ進み、即時 Scan にはしない。

## 3. 共通 App Shell

```text
┌───────────────────────┬───────────────────────────────────────────┐
│ Project switcher      │ Project / Site / active NetworkContext    │
│                       │ Last scan: completed with warnings         │
│ Overview              │ [Search]                         [Scan…]   │
│ Topology              ├───────────────────────────────────────────┤
│   Physical             │ Workspace content                           │
│   L2 Logical           │                                             │
│   L3 Logical           │                                             │
│   Combined             │                                             │
│ IPAM                  │                                             │
│ Changes               │                                             │
│ Unresolved            │                                             │
│ Scans                 │                                             │
│ Scope & Profiles      │                                             │
└───────────────────────┴───────────────────────────────────────────┘
```

### 3.1 常設要素

| 要素 | 内容・操作 |
| --- | --- |
| Project switcher | Project の切替と作成。切替前に未保存の Diagram Layout がある場合だけ保存・破棄・キャンセルを選べる。 |
| Context breadcrumb | `Project / Site / NetworkContext`。表示対象の絞込みであり、Scope を変更しない。 |
| Last scan status | `completed`、`completed with warnings`、`running`、`interrupted` を文章とアイコンで示す。warning は Unresolved / Diagnostics へ遷移する。 |
| Global search | Device、hostname、IP、MAC、VLAN、Subnet を横断検索する。結果には種別、Context、状態、確度を表示する。 |
| Scan | `Scan…` を押すと Preflight を開く。Scope、Profile、権限、対象数、Active Provider を確認しない限り開始できない。 |

## 4. Topology Workspace

### 4.1 レイアウト

```text
┌──────────────────────┬────────────────────────────────────┬──────────────────┐
│ View / filter rail   │ Canvas                             │ Inspector        │
│ [Physical][L2][L3]   │  Gateway ─ CoreSW ─ AccessSW       │ selected entity  │
│ Layers / legend      │                    ╲               │ state / evidence │
│ Search in view       │                 Shared Segment     │ related items    │
│                      │                   ├ PC             │ actions          │
│                      │                   └ Printer        │                  │
└──────────────────────┴────────────────────────────────────┴──────────────────┘
```

- 画面幅 1,280 px 以上は filter rail / Canvas / Inspector の三列とする。1,024–1,279 px は Inspector を折り畳み式にする。1,024 px 未満は Canvas と Inspector を同一領域で切り替える。
- Canvas は pan / zoom / fit-to-view / selection と、Diagram Layout の node position を変えるドラッグ移動を直接操作として持つ。位置変更は `layout.save` で保存し、Network Data を変更しない。ドラッグで Device を別の Device に接続したり、Edge を生成・削除してはならない。
- `Physical`、`L2 Logical`、`L3 Logical`、`Combined` は同じ Entity を別 projection で描く。View 切替は Network Data を変更しない。
- Combined View の Layer filter は `Physical`、`VLAN`、`Subnet`、`Routing`、`Wireless Clients`、`IP`。大量 Entity では初期状態を Physical + Subnet に限定し、追加 Layer は利用者が選ぶ。

### 4.2 構成図の表現規則

| 表現 | 意味 | 必須ラベル |
| --- | --- | --- |
| 実線 | `known` または `user_confirmed` の関係 | 取得元または確認済み状態 |
| 破線 | `inferred` の関係 | `Inferred` と confidence |
| 点線 + `?` | `unknown` / `SharedSegment` | `Unknown` または unknown reason |
| muted 表示 | `stale` / `missing` / `offline_candidate` | 最終観測時刻 |
| 二重線 | LAG / logical aggregate | LAG 名と member 数 |

色だけで意味を伝えない。線種、`?`、テキスト、アイコンを併用する。Reachability と Route は Physical View の接続線として表示しない。

### 4.3 Inspector

Entity を一つ選択すると Inspector を開く。最初の表示は次の順とする。

1. canonical name、type、status、assertion state、confidence、最終観測時刻
2. 関連 NetworkAdapter / Interface / IP / VLAN / Link / Subnet
3. 「この判定の根拠」— Evidence Inspector を開くボタン
4. Unresolved / conflict / stale の注意
5. 許可された手動操作（名前、タグ、UserOverride、Merge/Split の候補）

`UserOverride`、Merge、Split は Inspector から開始するが、変更内容、対象、既存の根拠、監査記録に残ることを確認する dialog を必須とする。自動データを直接編集する UI を置かない。

### 4.4 NetworkAdapter / NIC Inspector

Device Inspector には `Network adapters` セクションを置く。既定は Adapter 単位の表で、行を選ぶと関連する論理 Interface と Evidence を展開する。

| 列 | 表示内容 |
| --- | --- |
| Kind | `Physical` / `Virtual` / `Unknown`。色だけでなく文字で示す。 |
| Adapter | canonical name と stable OS ID が取得できた場合の短縮表示。 |
| Link | admin / oper / carrier、speed、MTU。`unknown` はハイフンではなく理由を Inspector に出す。 |
| MAC | 正規化 MAC と locally administered の表示。 |
| Interfaces | VLAN、bridge、bond/LAG、tunnel、loopback を含む関連 Interface 数。 |
| Source | `Local OS` / `SNMP` / `User confirmed` と最終観測時刻。 |

Physical NIC の詳細には vendor、model、bus location、driver、firmware、maximum speed を、取得できた Evidence とともに表示する。Virtual NIC の詳細には virtualization platform、virtual switch、parent adapter、関連 Interface を表示する。Phase 1 の Local OS Provider が取得できない仮想化属性、または remote SNMP Interface の driver / PCI / firmware は `not_observed` とし、「この情報源では取得不可」と表示する。空欄や故障と誤認させない。

Topology Canvas では全 NIC を node として常時描画しない。Device node を選択したときの Inspector と `Interfaces` filter で表示する。Virtual Interface / loopback を Canvas に重ねるのは Combined View の任意 Layer とし、Guest/Host の物理接続が Evidence なしに描かれないようにする。

### 4.5 Device Inventory と詳細 Drawer

`Devices` は、ネットワーク全体を素早く把握し、個別 Device の詳細へ入る一覧画面である。UniFi の機器一覧と詳細パネルのような操作性を提供するが、製品固有の意匠や未取得のテレメトリを模倣しない。

```text
┌──────────────────────────────────────────────────────────────────────────┐
│ Devices  [Search] [Type] [Status] [Site] [Context]        34 devices     │
├───────────────────────────────────────────────────────┬──────────────────┤
│ Name / type / vendor / connection / IP / state / seen │ Device Drawer    │
│ CoreSW01 · L3 switch · Cisco · Gi1/0/48 → Gateway01   │ Overview         │
│ Notebook01 · PC · Apple · Wi‑Fi via AP01 (inferred)   │ Connection       │
│ Printer02 · Printer · Unknown · Shared Segment (?)     │ IP & NIC         │
│                                                         │ Evidence         │
└───────────────────────────────────────────────────────┴──────────────────┘
```

- 一覧列は `Name`、`Type`、`Vendor`、`Connection`、`IP address`、`Status`、`Last seen` を基本とする。列は利用者が表示・順序を変更できるが、全件を一度に IPC 取得しない。
- `Connection` は根拠付きに表示する。例は `Known: CoreSW01 Gi1/0/48`、`Inferred: Shared Segment (73%)`、`Wi‑Fi: AP01 / SSID`、`Unknown`。FDB、ARP、同一 Subnet、ping 成功だけを direct port 接続として表示しない。
- 行を選ぶと Device Drawer を開く。Drawer は `Overview`、`Connection`、`IP & NIC`、`Evidence` のタブを持つ。`Connection` から Topology の該当 node を中心に表示できる。
- Phase 1 で表示する回線情報は、link state、negotiated speed、取得済み counter の**単一 Scan 時点の値**までとする。時系列 traffic chart、port utilization chart、監視・通知は実装しない。
- Wi‑Fi signal / RSSI / SNR は、実行端末自身について Local OS Provider が取得できた場合だけ Phase 1 で表示してよい。リモート AP 配下 Client の品質、roaming、利用量は Wireless Controller Provider を導入する Phase 2 まで `not_observed` と明示する。未取得時は空のゲージや 0 値を描かない。
- `IP & NIC` タブの IP、Reservation、Conflict Candidate は、該当する IPAM の IP Inspector または Subnet filter へ直接遷移できる。遷移先も同じ `NetworkContext` を維持する。
- Device 設定を変更する `Settings` タブは Phase 1 に置かない。利用者が可能な変更は、UserOverride、tag、Reservation、Unresolved disposition のみとする。

### 4.6 Topology の階層・探索操作

Physical / L2 / L3 / Combined の Canvas には、Device type ごとの**汎用** icon、name、状態、根拠状態を表示する。ベンダーロゴや製品の画面を複製しない。

- `Find in topology` は Inventory、IPAM、Changes、Unresolved、Drawer から使用でき、選択 Entity を center / highlight する。対象が collapse された Site / Subnet / SharedSegment / DeviceGroup / LAG 内にある場合は、一時的な `reveal` 状態で祖先を展開する。非表示 Layer の Entity は一時的に Layer を有効にして対象を表示する。元の filter / collapse 設定は保存せず、reveal を閉じると復元する。いずれも Network Data と Diagram Layout を変更しない。
- `Collapse` は Site、Subnet、SharedSegment、DeviceGroup、LAG を対象にする。collapse された集合には Device 数と unresolved 数を表示する。
- port 単位の線ラベルは、端点 Interface が解決済みの場合だけ表示する。未解決なら link は `Unknown port` と Evidence Inspector への導線を持つ。
- クライアント、AP、WAN、Subnet の表示は Layer filter で切り替える。Wi‑Fi / WAN の詳細が未観測なら、表示しないか Unknown とし、推測の icon や線を足さない。

## 5. IPAM

IPAM は一覧優先の画面とする。構成図を常時並べず、選択した IP / Subnet の関連だけを Inspector に表示する。

```text
┌────────────────────────────────────────────────────────────────────┐
│ IPAM  [Search IP, MAC, hostname]  [Context] [Status] [VLAN]        │
├───────────────┬────────────────────────────────────┬───────────────┤
│ Subnet tree   │ Result table                       │ Inspector     │
│ 10.10.0.0/16  │ IP / status / hostname / device    │ evidence      │
│ └ 10.10.10/24 │ last seen / confidence / conflict  │ assignments   │
└───────────────┴────────────────────────────────────┴───────────────┘
```

- Result table は cursor pagination と stable sort を使う。Subnet 全アドレスを表として作らない。
- `available`、`probably_available`、`unknown` は文章と tooltip で差を説明する。Probe 未応答を空き IP のように見せない。
- IP conflict は error 表示だけにせず、観測 MAC、Virtual Gateway 候補、時刻、Evidence を表示する。
- Reservation の追加・編集は UserOverride として扱い、Scope / Context / IP / 説明を確認して保存する。

## 6. Changes と Unresolved

### 6.1 Changes

Changes は比較対象となる二つの completed Session を上部で明示する。`coverage_reduced`、資格情報失敗、Scope 縮小、権限不足がある差分は「削除」ではなく抑制理由付きで表示する。

一覧の列は `kind`、`entity`、`before`、`after`、`evidence`、`baseline`、`state` とする。フィルタは Device / Interface / Link / VLAN / IP / MAC / Wireless / WAN とする。行を選ぶと影響範囲と Evidence を開く。

### 6.2 Unresolved

Unresolved は問題チケットではなく、情報不足または人の判断待ちを集める Queue である。

| 種別 | 初期アクション |
| --- | --- |
| Unknown Device / Vendor | 根拠を見る、名称・タイプを Override |
| Low-confidence Link / SharedSegment | 関連 Interface と Evidence を見る、確認済み Link を追加 |
| Identity conflict / Merge candidate | Identity と時系列を比較、Merge / Split / 却下 |
| Duplicate IP candidate | MAC、Virtual Gateway 候補、観測地点を比較 |
| Missing / offline candidate | Scope、coverage、最終観測、diagnostic を確認 |

解決、却下、保留は理由を要求し、Audit Trail に追加する。却下済みの同一根拠が再度現れた場合は新規ではなく再発として表示する。操作完了時は状態、理由、対象、Evidence snapshot を Inspector で確認できる。

## 7. Scan flow

```text
Scan… -> Preflight -> Start -> Progress -> Completed / Warnings -> Review changes
                   └-> Cancel request -> Cancelled / Interrupted
```

### 7.1 Preflight dialog

Preflight は Scan の唯一の開始画面とし、次を表示する。

- Scope 名、include / exclude、解決 host 数、最大 host 数、再帰深度
- Profile、Active / Passive Provider、TCP port profile、SNMP target 数
- OS Provider Capability、権限不足で skip される収集項目
- 使用する Credential **reference 名のみ**。secret、community、token は表示しない。
- 想定する timeout / retry / request limit と「Scope 外は Probe しない」こと

開始ボタンは Scope validation と host 上限が成功した時だけ有効にする。`no_privilege` は警告であっても開始可能だが、影響する Discovery Quality を明示する。

#### LocalNetwork capability の確認

Preflight / Scope 画面の「ローカル capability を確認」は、Tauri desktop backend の OS API を使った Interface/address の受動列挙だけを実行する。provider 名、OS、Interface 件数、`available` / `no_privilege` / `unsupported` を表示し、route、ARP/NDP、ICMP、raw packet は `unsupported` と明示する。取得した Interface 名・アドレスは折りたたみの詳細操作でだけ表示し、ここで Scan Session を開始しない。ブラウザでは `desktop_backend_unavailable` と表示し、モック値を返さない。

### 7.2 Progress screen

Session ID、Scope / Profile snapshot、現在の Provider、完了 target 数、warning 数、cancel request 状態を表示する。未確定の topology を完成図として表示しない。途中結果は `provisional` と表示し、完了後に Changes / Unresolved へ遷移できる。

## 8. Empty、Error、権限不足の状態

| 状態 | 表示 | 主操作 |
| --- | --- | --- |
| Project がない | Project の目的と作成 | Project を作成 |
| Scope がない | Scan 不可の理由 | Scope を作成 |
| まだ Scan していない | 空の Canvas と開始手順 | Preflight を開く |
| `no_privilege` | 影響する Provider と取得できない情報 | OS 設定を確認 / そのまま Scan |
| `authentication_failed` | Credential reference と対象、秘密を除く診断 | Scope の Credential 割当を変更 |
| `coverage_reduced` | Diff を抑制した理由 | Session / Scope を比較 |
| `interrupted` | 前回結果を保持した旨 | Session 詳細 / 再 Scan |
| `credential_required` | 復元した Project で Secure Storage の参照先がない旨 | Credential を再割当。自動 Scan はしない。 |

Error message は error code、利用者向け説明、correlation ID を持ち、raw payload、secret、OS command を表示しない。

## 9. アクセシビリティとキーボード

- Canvas を除く全操作はキーボードだけで利用可能にする。Canvas には fit-to-view、選択 Entity 移動、Inspector を開く同等のキーボード操作を提供する。
- すべての state は色以外のテキスト・線種・アイコンで区別する。tooltip にだけ重要情報を置かない。
- Focus を隠さず、modal は開いたときにフォーカスを移し、閉じたときは起点へ戻す。
- データ更新は `aria-live="polite"` で要約だけを知らせる。Scan progress の target ごとの更新を逐次読み上げない。
- 画面表示の timezone と Evidence の UTC 原時刻を Inspector から確認できるようにする。

## 10. Phase 1 画面受入条件

1. Physical / L2 / L3 / Combined / IPAM / Changes / Unresolved をナビゲーションから開ける。
2. Unknown / Inferred / User Confirmed / Stale を、色覚に依存せず判別でき、Evidence Inspector へ到達できる。
3. Preflight を通らずに Scan を開始できず、Scope / Provider / target count / capability を確認できる。
4. Scan 中断、資格情報失敗、権限不足、coverage reduced が削除された Device のように表示されない。
5. 大規模 fixture で table は pagination、Canvas は filter / collapse / virtualization を使用し、全件同時描画しない。
6. Reservation、UserOverride、Merge/Split、Unresolved の解決には確認・理由・Audit Trail がある。
7. 1,024 px 幅の Inspector 折り畳みと、キーボードのみの主要フローを自動または管理済み手動試験で確認する。
8. Local OS fixture では物理 NIC と仮想 NIC の Adapter / Interface 関係、remote SNMP fixture では driver / PCI 情報を `not_observed` と表示でき、いずれも根拠のない Guest/Host Link を生成しない。
9. Canvas 上で node の位置だけを保存でき、ドラッグ操作で Link / VLAN / IP / Evidence を変更できない。Unresolved の解決・却下・保留は理由と Evidence snapshot を Audit Trail に残す。
10. Device Inventory で cursor pagination と filter が機能し、行の `Connection` が Known / Inferred / Unknown を区別する。Drawer と `Find in topology` は同じ Entity を指し、未取得の Wi‑Fi 品質・traffic を 0 や正常値として表示しない。
