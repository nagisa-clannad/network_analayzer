# ネットワーク構成自動可視化・IPAMツール 統合仕様書 v0.4.11

- 状態: Phase 1 foundation / LocalNetwork capability 実装済み（Discovery scan Provider / Resolver は未実装）
- 更新日: 2026-09-06
- 本文書の対象: ローカルで動作するクロスプラットフォームのデスクトップアプリケーション
- 前提: v0.3 の「取得できない情報を推測で確定しない」という方針を継承し、曖昧だった安全性、時系列、差分、識別子の要件を規範化する。

## 1. 製品目的と境界

本製品は、利用者が許可したネットワーク範囲から機器、接続、論理ネットワーク、IPアドレスを収集し、根拠付きの構成図とIPAMをローカルで管理する。Windows 11、macOS、Linux を対象とし、Tauri 2 / React / TypeScript / Rust / SQLite を基本技術とする。

MVP の目的は「もっともらしい図を描くこと」ではない。収集できた事実と推論を区別し、不明な箇所を不明のまま表示して、再スキャン後にも追跡可能な構成管理を完成させることである。

### 1.1 明示的な非対象（Phase 1）

- 機器設定の変更、SNMP SET、SSH/CLI の設定コマンド実行、API による変更をしない。
- パスワード推測、総当たり認証、許可外の CIDR、無差別 UDP スキャン、パケットの改変・注入をしない。
- パケットキャプチャ、トラフィック解析、脆弱性診断、監視通知サービス、クラウド同期を実装しない。
- ping、TCP 接続成功、同一サブネット、FDB、ARP のいずれからも直接物理接続を確定しない。

上記を越える機能は、別フェーズの利用者承認・脅威モデル・受入基準なしに追加してはならない。

## 2. 規範と用語

本文書の **MUST** は必須、**SHOULD** は正当な理由がない限り実施、**MAY** は任意とする。

以下の概念を混同してはならない。

```text
Observation != Evidence != Inference != User Confirmed Fact
Physical Link != L2 Relationship != L3 Adjacency != Route != Reachability
Device != Interface != MAC Address != IP Address
```

### 2.1 情報状態

すべての自動生成属性・リンク・IP割当には、少なくとも次を持たせる。

| 属性 | 意味 |
| --- | --- |
| `assertion_state` | `known` / `inferred` / `unknown` / `user_confirmed` |
| `availability_state` | `current` / `stale` / `missing` / `archived` |
| `unknown_reason` | `not_observed` / `unsupported` / `not_configured` / `authentication_failed` / `permission_denied` / `timeout` / `unreachable` / `out_of_scope` / `ambiguous` |
| `confidence` | 0.0–1.0。表示は証拠一覧と併記し、値だけを正しさの証明にしない。 |

`unknown` はエラーでも否定でもない。特に `authentication_failed`、`timeout`、`out_of_scope` を単なる「情報なし」に畳み込まない。理由が異なれば利用者の次の行動も、差分判定も異なるためである。

## 3. 安全な探索の契約

### 3.1 Project Scope

Project は必ず少なくとも一つの `DiscoveryScope` を持つ。Scope は CIDR、単一 IP、Seed Device、または明示した Site / NetworkContext であり、以下を保存する。

```text
DiscoveryScope
├─ project_id
├─ include_targets
├─ exclude_targets
├─ active_discovery_allowed
├─ allowed_providers
├─ max_hosts
├─ max_depth
├─ credential_scope_ids
├─ approved_at
└─ approved_by (Phase 1 は local_user)
```

- Scan 実行前に、解決済みの対象数・アクティブ操作の種類・上限を Preflight 画面で表示し、Scope 外のキュー項目を拒否する。
- 初期値は 1,024 host、再帰深度 3、同時接続 16 とする。これを超える場合、対象数と負荷を表示した明示的な Scope 更新を必要とする。
- `10.0.0.0/8`、`172.16.0.0/12`、`192.168.0.0/16` 全域のような広域 Scope は、警告だけでなく host 上限を超える限り実行できない。
- DNS、LLDP、FDB、ルーティングテーブルから得た対象も Scope 内でなければ保存は可能だが、再帰 Probe は行わず `out_of_scope` と記録する。
- Active Discovery は ICMP、許可済みポートへの TCP connect、SNMP GET/GETBULK、DNS 問合せに限定する。TCP はペイロードを送らず、ポート集合は Profile で明示する。
- 再試行、timeout、packet/request rate、SNMP walk 件数、キュー件数には Profile ごとの上限を設ける。停止要求を受けたら新規 Probe を発行せず、進行中処理を期限内に中断する。

### 3.1.1 OS 権限と Provider Capability

- ICMP、生パケット、ARP/NDP、ルート取得などの OS 権限は Provider ごとに `available` / `no_privilege` / `unsupported` を Preflight で判定・表示する。Scan の途中で権限昇格を要求してはならない。
- `no_privilege` の Provider は `permission_denied` 診断を残してスキップする。アプリは管理者権限を前提にせず、権限不足を ICMP 未応答や Device の不在として扱ってはならない。
- OS API、または固定引数だけを許す管理済み helper を実装に用いてよい。ネットワーク由来の値をシェル文字列へ連結した subprocess 実行、任意コマンド実行、無言の権限昇格は禁止する。
- 必要な OS 権限を利用者があらかじめ許可・設定する場合は、その手順、対象 Provider、失敗時のフォールバックを PlatformNetworkProvider の実装ごとに文書化する。

### 3.2 読み取り専用と認証情報

- Discovery Provider は読み取り専用でなければならない。SNMP は GET/GETNEXT/GETBULK のみ、SSH/CLI・Vendor API は後続フェーズでも read-only コマンド/API を allowlist 方式で指定する。
- SNMP v2c community、SNMP v3 の認証・暗号鍵、SSH 秘密鍵とパスフレーズ、API token を SQLite、JSON export、ログ、クラッシュレポートへ平文保存してはならない。
- 秘密情報は OS の Secure Storage に格納し、DB には参照 ID と Scope だけを持つ。Credential の表示、コピー、export はしない。
- Credential は `Device > Subnet > Site > Project default` の順に**一つだけ**解決する。候補を順番に試す総当たりフォールバックは禁止する。解決 Credential が認証に失敗した場合は、その target + transport への自動再試行を同一 Session で停止し `authentication_failed` を記録する。別 Credential を試すには、利用者が明示的に Scope の割当を変更して新しい Session を開始する。
- 試行した Credential ID、成功/失敗種別、対象を監査記録に残す。ただし秘密値は一切残さない。Provider はロックアウト・rate-limit を示す応答を専用 diagnostic として扱い、当該 target の後続認証を停止する。
- SNMP v2c は平文であることを登録時に表示する。v3 を Phase 2 の優先機能とする。

### 3.3 収集データの信頼境界

ネットワークから得る hostname、SNMP string、LLDP/CDP 値、DNS 名、説明欄、Vendor API 応答、JSON import はすべて非信頼入力である。UI 表示、検索、export、ログではエスケープ・長さ制限・スキーマ検証を行う。外部値をシェル、SQL、テンプレート、ファイルパス、SQL migration として解釈してはならない。

## 4. アーキテクチャと処理フロー

```text
Scope + Profile + Credential reference
  -> Scan plan / admission control
  -> DiscoveryProvider[]
  -> immutable Raw Observation
  -> Normalizer
  -> Evidence
  -> Entity Resolver
  -> Inference Engine
  -> current-state projection / Topology / IPAM / Diff
  -> User Override (表示優先のみ)
```

### 4.1 Provider 契約

`DiscoveryProvider` は対象・Scope・Profile・Credential reference を入力とし、`Observation[]` と `ProviderDiagnostic[]` のみを返す。Provider が Device、Topology、IPAM、Diagram を直接更新してはならない。

各 Observation は次の共通メタデータを必須とする。

```text
observation_id, scan_session_id, provider_id, provider_version,
source_device/interface, target, network_context, raw_kind, normalized_kind,
payload_hash, collected_at, observed_at, received_at, scope_id,
credential_reference_id?, transport, result, diagnostic_code, raw_payload_ref?
```

- `collected_at` はアプリが収集した時刻、`observed_at` は情報源が示す観測時刻、`received_at` は受信時刻とする。情報源が時刻を示さない場合は `observed_at = collected_at` とし、推測で過去時刻を作らない。
- raw payload は保持期間内に Evidence Inspector から参照可能とする。秘密値を含み得る箇所は保存前にマスクし、payload hash は改ざん・再処理の検出に用いる。
- Provider の例外は Scan 全体を失敗させず、診断として記録する。ただし DB 整合性、Scope 違反、暗号化/資格情報の不正はフェイルクローズとする。

### 4.2 Scan Session と部分失敗

```text
ScanSession
├─ id, project_id, started_at, ended_at, status
├─ scope_snapshot, profile_snapshot, provider_versions, seed_snapshot
├─ request_limits, counters, coverage_summary
├─ baseline_session_id, cancellation_reason
└─ warnings / diagnostics
```

`status` は `planned` / `running` / `completed` / `completed_with_warnings` / `failed` / `cancelled` / `interrupted` とする。

- Observation は Session に紐付けて追記する。過去 Observation を上書きしない。
- `failed` または `cancelled` Session は調査用に表示できるが、前回の Current State を `missing` に遷移させず、削除・撤去候補の Diff を発生させない。
- プロセス終了、OS 再起動、DB lock timeout 等で終了処理を完了できなかった `running` Session は、次回 Project open 時に transaction 内で `interrupted` に遷移させる。`interrupted` は `failed` / `cancelled` と同様に Missing / Removed の根拠にしない。
- `completed_with_warnings` は、対象範囲・使用 Provider・認証到達性が baseline と比較可能な場合だけ、「未観測」を Missing 判定の根拠にできる。比較不能なら `coverage_reduced` として差分を抑制する。
- 再スキャンを idempotent に扱い、同一 Evidence の重複が Device / Link / IP を重複生成しないようにする。

## 5. 正規化データモデル

全 Entity は UUID、`first_seen_at`、`last_seen_at`、Evidence 参照、表示上の `assertion_state` を持つ。表示位置・フィルタ・レイアウトは Diagram 側にのみ持ち、Network Data と分離する。

### 5.1 基本 Entity

```text
Project -> Site -> NetworkContext
                     ├─ Device -> NetworkAdapter -> Interface -> LAG / DeviceGroup
                     ├─ VLAN -> VLANMembership / LinkVLANMembership
                     ├─ Subnet -> IPAddress -> IPAssignment / IPPool
                     ├─ Route / RouteGroup / WanCircuit
                     ├─ WirelessNetwork -> BSSID -> WirelessAssociation
                     └─ Observation -> Evidence -> Inference -> UserOverride
```

- `NetworkContext` は `project_id`, `site_id`, `vrf/namespace`, `tenant`, `name` を持つ。IP と Subnet の一意性は `NetworkContext + address/prefix` で判定する。
- Device は `canonical_name`, hostname, type, vendor, model, OS, serial, asset tag, virtual, status を持つ。type は `unknown` と `other` を必須にし、未知の enum 値を読める実装にする。
- `NetworkAdapter` は Device 上の物理または仮想 NIC、`Interface` は VLAN subinterface、bridge、bond、tunnel、loopback を含む論理的な通信面を表す。NIC 本体と Interface を混同しない。
- Interface は device、`network_adapter_id?`、name、alias、`ifIndex`、MAC、admin/oper state、speed、MTU、parent、LAG、時刻を持つ。`ifIndex` は Device 内でのみ一意として扱う。
- DeviceIdentity は serial、SNMP engine ID、LLDP chassis ID、MAC、sysName、sysObjectID、management IP 等を時系列付きで保存する。
- `PhysicalLink` は Interface 間の物理接続、`SharedSegment` は未知の L2 中間部分、`VLANMembership` は Interface と VLAN、`LinkVLANMembership` は PhysicalLink と VLAN を表す。これらを一つの Edge 型に統合しない。

### 5.1.1 NetworkAdapter（物理 NIC / 仮想 NIC）

```text
NetworkAdapter
├─ id, device_id, canonical_name, stable_os_id?
├─ adapter_kind: physical / virtual / unknown
├─ mac_address_id?, admin_status, oper_status, carrier_status
├─ media_type, link_speed, maximum_speed, mtu
├─ parent_adapter_id?, virtual_switch_name?, virtualization_platform?
├─ bus_type?, bus_location?, hardware_vendor?, hardware_model?, serial_number?
├─ driver_name?, driver_version?, firmware_version?
├─ first_seen_at, last_seen_at, source_visibility
└─ evidence
```

- `physical` は PCI/USB/Thunderbolt などの実体 NIC、`virtual` は Hyper-V/VMware/KVM/VirtualBox の vNIC、OS bridge、veth 等を表す。列挙外の platform 値は `unknown` と raw value を保持する。
- `NetworkAdapter -> Interface` は一対多を許容する。VLAN subinterface、bridge、bond/LAG、tunnel、loopback は通常 Interface として表し、必ずしも独立した NIC を作らない。
- `parent_adapter_id`、`virtual_switch_name`、`host_device_id` 相当の関連は Evidence がある場合だけ保存する。Guest vNIC と Host の物理 NIC を、同じ MAC、名称、IP 帯だけから自動接続してはならない。
- PhysicalLink は従来どおり Interface 間の関係である。Adapter の存在、link state、carrier、speed だけから外部の物理接続線を生成しない。
- `PhysicalLink` を生成できるのは、Evidence が physical port と識別した Interface 同士だけとする。VLAN/SVI、bridge、bond/LAG 本体、tunnel、loopback、virtual NIC/veth 等の論理専用 Interface を endpoint とする PhysicalLink を推論・生成してはならない。LAG は member Interface の根拠付き PhysicalLink を集約して表示する。
- hardware、driver、firmware、bus location は `source_visibility = local_os` の場合に取得を試みる。SNMP IF-MIB で得られる remote Interface は `ifType`、MAC、status、speed 等を表示できるが、PCI/driver/firmware は `unknown` / `not_observed` とし、空値を「存在しない」と表示しない。
- 仮想化基盤からの Guest/Host 対応、vSwitch / port group、container network の詳細取得は Phase 2 の Hypervisor / SSH / Vendor Provider で扱う。Phase 1 は Local OS の virtual NIC と、remote の論理 Interface を根拠付きで表示する。

### 5.2 識別・統合ルール

- IP、hostname、MAC OUI、ポート開放、単一の一時 MAC だけで Device を自動統合してはならない。
- serial、SNMP engine ID、安定した chassis ID の一致を強い根拠とする。矛盾する強い識別子は `identity_conflict` として Unresolved に出し、自動マージしない。
- Merge/Split は元 Observation・Evidence・ID alias・UserOverride を失わず監査可能かつ取り消し可能にする。UserOverride は自動値を破壊せず、表示優先順位だけを変更する。
- 表示優先順位は `UserOverride > UserConfirmed > current strongest Evidence > Inference > fallback` とする。Override には作成者、理由、時刻、対象 Session を保存する。

### 5.3 トポロジー確度

- LLDP/CDP でポートまで対応付いた直接隣接は `known` 候補とする。片側のみ、port ID 変換不確実、古い情報は Evidence と freshness に応じて `inferred` または `stale` に下げる。
- FDB + ARP は Endpoint がある L2 Segment への所属または位置推論の根拠であり、直接 `PhysicalLink` の根拠にはならない。中間機器があり得る場合は `SharedSegment` を生成する。
- ICMP/TCP 成功、Route、同一 Subnet は Reachability / L3 の根拠であり、Physical View の線を生成しない。
- VLAN と Subnet は独立 Entity とし、対応は `VLANSubnetRelation` に confidence と Evidence を付けて表す。VLAN ID は Project 全体ではなく `NetworkContext + Site` 内で扱う。
- Stack、Virtual Chassis、HA pair、LAG、MLAG、STP は通常 Device/PhysicalLink と別モデルにする。Phase 1 は LAG と `DeviceGroup` の拡張可能な境界まで、STP/MLAG の解析は Phase 2 以降とする。

### 5.4 IPAM、IPv4、IPv6

- Subnet の全アドレスを事前生成しない。`IPAddress` は観測済み、予約、gateway、DHCP/DNS 由来、手動管理対象のみとし、使用可能数は動的に計算する。
- IP 状態は `in_use` / `reserved` / `available` / `probably_available` / `unknown` / `conflict_candidate` / `deprecated` とする。Probe 未応答を `available` と判定してはならない。
- IP conflict は複数 MAC の観測だけで障害確定しない。VRRP/HSRP/CARP、anycast、cluster、proxy ARP、NAT を候補として `conflict_candidate` に留める。
- IPv6 は Phase 1 から binary address、address family、canonical display、prefix、scope を保持する。global/ULA の一意性は `NetworkContext + address`、link-local の一意性は `NetworkContext + address + link_scope_key` とする。Local OS から直接利用する link-local は interface/scope zone を必須とする。一方、SNMP 等でリモート機器から得た link-local の local scope zone は null を許容し、判明していれば source Device Interface を `link_scope_key` に用いる。別 interface の同じ link-local を衝突と判定しない。temporary / privacy address は安定 DeviceIdentity として用いない。
- IPv6 の MVP Discovery は Local OS Neighbor と明示 Scope 内の対象に限定する。IPv6 全空間・プレフィックスの全列挙、SLAAC/DHCPv6/RA の完全解析は Phase 2 とする。

### 5.5 Wi-Fi、WAN、Route

- Wi-Fi Client と AP の関係は `WirelessAssociation` とし、有線 `PhysicalLink` に変換しない。SSID、BSSID、Radio、VLAN、RSSI、roaming は独立の時系列情報とする。
- `WanCircuit` は Device Interface、provider、circuit ID、media、帯域、public IP、gateway、status、role、cost、priority を持つ。role は表示補助であり、経路の正は Route / RoutingPolicy である。
- Route は `NetworkContext`, Device, destination, next hop, outgoing interface, protocol, metric, distance, VRF, active, Evidence を持つ。Reachability は Route と別の観測結果として保存する。

## 6. Evidence、推論、鮮度

Evidence は `source_type`, target Entity/attribute, value, Observation reference、confidence、`observed_at`, `expires_at`, metadata を持つ。Inference は `rule_id`, `rule_version`, input Evidence IDs, confidence, generated_at を持ち、ルール更新時に再計算可能にする。

- Evidence の寿命は情報源ごとのポリシーで管理する。LLDP current、FDB、ARP、DNS、手動入力を同じ期限にしない。
- 時刻不整合、Scope 外、失敗した資格情報、Provider の不完全応答は confidence を上げる根拠に使わない。
- Evidence Inspector は、属性・リンク・IP 判定ごとに「結論、状態、unknown reason、使用した根拠、収集 Session、時刻、推論 rule version」を表示する。
- `Raw Observation` は既定 30 日、Current State と Change History は保持する。保持期間・マスク規則は Project 単位で設定できる。削除後は参照不能である旨を Evidence Inspector に表示する。

## 7. UI と操作要件

必須 View は Physical、L2 Logical、L3 Logical、Combined、IPAM、Changes、Evidence Inspector、Unresolved Items とする。Wireless / WAN は Phase 1 で最小の Entity 表示、詳細 View は Phase 2 とする。

- Physical View: Device、Interface、PhysicalLink、SharedSegment、LAG、Stack、Media を表示する。
- L2 View: VLAN、tagged/untagged、trunk、L2 relationship を表示する。STP state は取得できた時のみ表示する。
- L3 View: Subnet、Gateway、SVI、Router、Route、VRF、WAN を表示する。
- Combined View: Layer 単位の on/off、filter、group、collapse を持つ。大量 Entity を一度に描画しない。
- Unknown / Inferred の線・ノードは視覚的に `known` と区別し、理由と Evidence Inspector へ遷移できる。
- Unresolved Items は identity conflict、merge candidate、low-confidence link、SharedSegment、duplicate IP candidate、missing device、unknown VLAN mapping、未識別 WAN、unknown wireless client を分類・解決・却下できる。
- 手動変更は Undo/Redo、Audit Trail、理由、時刻を持つ。自動再スキャンで手動変更を消さない。

## 8. 永続化、移行、Import/Export

- SQLite は WAL、foreign key、有効な index、単一 writer 制御、integrity check を用いる。対象 index は DeviceIdentity、MAC、NetworkContext + IP、hostname、Device + ifIndex、Context + VLAN、last_seen、ScanSession、Evidence target とする。
- Schema migration は versioned かつ transaction 内で実行する。開始前に復元可能な backup を作り、migration failure 時は旧 DB を開ける状態に保つ。
- Retention による Raw Observation の削除は、Scan と同時に full `VACUUM` を実行しない。既定では削除後に incremental vacuum の可否を記録し、full `VACUUM` は backup、必要空き容量、DB lock の影響を確認した利用者の明示的な保守操作として実行する。
- Backup はデータベースと schema version を含むが、Secure Storage の秘密値は含めない。復元後は Credential を再選択する。
- Project open / backup restore 時は、DB の Credential reference と現在の OS Secure Storage を照合する。実体がない reference は `secret_missing` / `credential_required` として無効化し、当該 Scope の Scan を開始できない。restore / open 自体が Scan を自動開始してはならない。
- JSON import/export は schema version、Project ID、作成時刻、含めるデータ種別を持つ。Import は検証・プレビュー・ID collision の明示的な merge policy を必要とし、Credential、秘密の raw payload、OS keychain reference を含めない。
- Audit Trail は手動変更、Scope/Profile 変更、Credential reference の追加・更新・削除、migration、backup/restore を記録する。Phase 1 は `local_user` を actor とし、複数利用者の認可は Phase 3 の別要件とする。

## 9. Change Tracking

Diff は任意の二つの比較可能な completed Session の Current State projection を比較する。種類は Device、Interface、Physical、VLAN、IP、MAC、Wireless、WAN を含む。

```text
Device: added / missing / recovered / changed
Interface: added / removed / up-down
Physical: link added / link removed / confidence changed
VLAN: added / removed / access-native-tagged changed
IP: assignment changed / conflict candidate
MAC: move
Wireless: joined / left / roamed
WAN: circuit down / primary changed / default route changed
```

- Missing は「この Session で未観測」だけでは作らない。同等以上の Scope、Profile、Provider coverage、かつ当該事実を得る Provider の到達性が確認でき、既定の連続観測失敗閾値を満たすときだけ候補にする。認証到達性は、SNMP table など認証済み Provider が根拠の属性にだけ必要であり、ICMP/TCP/隣接情報だけで観測する Device の Missing 判定に要求しない。
- 前回到達可能だった Device が、同じ観測地点から成功した非認証 Provider（ICMP/TCP/隣接情報）で連続して到達不能なら、`offline_candidate` とする。これは削除・撤去の根拠ではない。Scope 縮小、Provider の権限不足、資格情報失敗、timeout だけでは Missing/Offline へ遷移させない。
- `active -> offline_candidate -> missing -> removed_candidate -> archived` のライフサイクルを用いる。完全削除は利用者の明示操作のみとする。
- Diff には baseline Session、coverage 比較結果、根拠、抑制理由を保存する。credential failure、timeout、Scope 縮小による差分は削除候補に見せない。

## 10. 非機能要件

| 項目 | Phase 1 要件 |
| --- | --- |
| 規模 | 1,000 Devices、50,000 Interfaces、100,000 observed IPs、10,000 VLANs、数百万 Observations を保存・検索可能にする。 |
| UI | 全 Entity の同時描画を禁止し、filter、group、collapse、virtualization を使う。 |
| 失敗許容 | ICMP/DNS/SNMP の一部失敗でも収集済みの結果は保持し、Session を warning として完了できる。 |
| 可観測性 | Application / Discovery / Diagnostic / Security-Audit ログを分離し、秘密値と raw secret を記録しない。 |
| プラットフォーム | Interface、Route、ARP/NDP、権限、Secure Storage を `PlatformNetworkProvider` の OS 実装に分離する。 |
| オフライン性 | ローカル DB、ローカル OUI DB、標準 MIB で主要機能を実行できる。OUI DB の版・更新日時・出典を Evidence に残す。 |

## 11. フェーズと完了条件

### Phase 1 — 安全な end-to-end MVP

実装する。

- Scope/Profile/Preflight、Local OS、ARP/NDP、ICMP、限定 TCP probe、DNS、OUI、SNMP v2c、LLDP、FDB、Route の Provider。
- Observation/Evidence/Inference/UserOverride/ScanSession/Diff、Device/Interface/Identity、PhysicalLink/SharedSegment、VLAN/Subnet/IPAddress/IPAssignment/IPPool、基本 WAN / Wi-Fi Entity。
- Physical/L2/L3/Combined/IPAM/Changes/Evidence/Unresolved UI、SQLite migration/backup、再スキャン。

完了条件は次をすべて満たすこと。

1. 利用者が Scope を作成し、Preflight が上限・Probe 種別を表示してから Scan できる。
2. SNMP/LLDP なしの fixture で、Endpoint/Subnet は表示し、物理構成を断定せず Unknown/SharedSegment と根拠を表示する。
3. SNMP + LLDP + FDB の fixture で、Interface/VLAN と根拠付きの topology を生成する。
4. 部分失敗、資格情報失敗、権限不足、cancelled scan が既存 Device を Missing/Removed にしない。一方、同じ観測地点からの連続した非認証到達不能は `offline_candidate` となり、削除候補にはならない。
5. UserOverride、Merge/Split、IP予約を保存・再起動後にも再現し、再スキャンで消さない。
6. Schema migration の backup/restore と integrity check が自動テストされる。
7. Scope 外の再帰探索、SNMP SET、未許可 port への TCP probe がテストで拒否される。
8. ICMP/ARP/NDP の OS 権限がない環境で、Preflight が Provider を `no_privilege` と表示し、誤った未応答・Missing を生成しない。
9. 不正文字、規格外の長さ、壊れた payload を含む Provider fixture を入力しても、アプリがクラッシュせず、値を安全に拒否またはエスケープして監査可能な diagnostic を残す。
10. `large-scale-topology` fixture で第10節の規模を読み込み、全件を同時レンダリングせず検索・filter・virtualization が機能し、操作不能にならないことを計測する。

fixture は少なくとも `home-no-snmp`、`office-snmp`、`lldp-multi-switch`、`vlan-trunk`、`unmanaged-segment`、`duplicate-ip`、`dual-wan`、`ipv6-link-local`、`partial-failure`、`malformed-provider-response`、`large-scale-topology` を含める。Provider の実機結果だけで Topology Engine をテストしてはならない。

### Phase 2 — 管理精度の向上

SNMP v3、ENTITY-MIB 強化、CDP、STP、LACP、Stack、MLAG/vPC、DHCP/DNS/mDNS/SSDP、IPv6 の詳細 Discovery、VRF Discovery、Wireless Controller、SSID/BSSID/Roaming、Merge/Split UI の拡充を対象とする。各 Provider は Phase 1 の Observation 契約と Scope 制約を満たす。

### Phase 3 — 高度・大規模環境

read-only SSH/CLI、Vendor API、PBR、NAT、Virtual Gateway、OSPF/BGP、SD-WAN、VXLAN/VNI、VPN、Hypervisor/Container、AWS/Azure/GCP、外部 IPAM 連携、複数利用者の認可を対象とする。書込み操作を扱う場合は、本仕様から独立した変更管理・承認・ロールバック仕様を先に策定する。

## 12. v0.3 からの主な補完

| 優先度 | 見落とされていた点 | v0.4 の対応 |
| --- | --- | --- |
| P0 | 許可される探索範囲とアクティブ操作の境界 | DiscoveryScope、Preflight、上限、read-only Provider 契約を必須化。 |
| P0 | 失敗・キャンセル・Scope 変更を削除と誤認する危険 | Session coverage と比較可能性を Diff の前提条件にした。 |
| P0 | `Unknown` が一種類で、設定不足と到達不能を区別できない | `unknown_reason` を正規化した。 |
| P0 | 観測時刻、根拠、ルール版が不足し再現できない | immutable Observation、Evidence 参照、rule version、時刻意味を定義した。 |
| P1 | IP/hostname/MAC だけの自動統合による誤マージ | 強い識別子、conflict、可逆 Merge/Split を明文化した。 |
| P1 | IPv6 link-local / privacy address の誤った一意性 | interface scope、temporary address、限定 Discovery を追加した。 |
| P1 | 秘密情報・JSON import・ネットワーク入力の安全境界 | Secure Storage、redaction、untrusted input、import policy を追加した。 |
| P1 | DB 移行失敗と復元、複数処理の整合性 | transaction、backup、integrity check、single writer を必須化した。 |
| P2 | Phase ごとの実装量が曖昧 | Scope、安全、部分失敗を含む実行可能な Phase 1 完了条件に固定した。 |

## 13. 実装開始時に固定するインターフェース

実装着手前に、次だけを schema と API 契約として固定する。それ以外の将来 Entity を先行して大量に実装しない。

1. `DiscoveryProvider` の入力・Observation・Diagnostic 契約
2. `DiscoveryScope`、Profile、ScanSession、coverage/comparability の意味
3. Observation → Evidence → Entity Resolver → Inference → UserOverride の参照関係
4. Device/Interface/Identity、Physical/L2/L3/IPAM の責務境界
5. 時刻、assertion state、unknown reason、confidence、Evidence expiry の共通型
6. schema migration、backup/restore、JSON import/export の versioning 契約

これにより、Phase 2/3 は既存の事実モデルに Provider と解析を追加して精度を上げられる。MVP を将来機能の仮実装で複雑化せず、かつ後から「分からないこと」を消さずに拡張できる。

## 14. 追加レビューで採用した補完

追加の独立レビューで出た指摘のうち、現行要件と矛盾せず実装判断を明確にするものを採用した。

| 優先度 | 採用した補完 | 反映先 |
| --- | --- | --- |
| P0 | OS 権限の Capability 表示と、権限不足を不在と誤認しないフォールバック | 3.1.1、11 Phase 1 条件 8 |
| P1 | 自動 Credential 総当たりの禁止と、認証失敗時の停止 | 3.2 |
| P1 | 到達不能を Offline とし、認証失敗・Scope 縮小を削除と誤認しない状態遷移 | 9、11 Phase 1 条件 4 |
| P1 | リモート由来 IPv6 link-local で local scope zone を強制しない一意性規則 | 5.4 |
| P1 | 壊れた Provider 応答を安全に処理する受入テスト | 11 Phase 1 条件 9 |
| P2 | 規模要件を検証する仮想化テストと、Raw Observation 削除後の DB 保守 | 8、11 Phase 1 条件 10 |

## 15. Phase 1 実装ベースライン

この節は、Phase 1 を実装可能にするための固定契約である。画面の見た目、ベンダー固有 MIB、Phase 2 以降の Entity はこの節の契約を破らない限り後から変更できる。

### 15.1 モジュール境界

```text
Frontend (React / TypeScript)
  -> typed Tauri IPC
Application Service
  -> Domain (Entity / Evidence / Inference / Diff rules)
  -> Provider Orchestrator -> DiscoveryProvider[]
  -> Storage Repository -> SQLite
  -> SecretStore / PlatformNetworkProvider
```

- Frontend は SQLite、OS API、Credential、Provider を直接呼ばない。Backend は UI の React Flow node/edge 型を返さない。Diagram projection は Domain Entity から生成する。
- `DiscoveryProvider` は Observation / Diagnostic だけを返す。Entity matching、current-state projection、Diff、Diagram 更新は Application Service / Domain の責務とする。
- Repository だけが SQLite を更新する。Provider からの値は型検証・正規化・マスク前に永続化してはならない。
- `SecretStore` は `store` / `get reference` / `delete reference` だけを公開する。秘密値を返す API は Provider Orchestrator の内部に閉じ、Frontend、export、Audit、Diagnostic へ渡さない。
- PlatformNetworkProvider は OS ごとの Interface、Route、ARP/NDP、権限 capability を実装する。Provider は OS コマンド出力の文字列形式へ直接依存しない。

### 15.2 Tauri IPC とイベント契約

IPC は schema から TypeScript / Rust の両方を生成するか、同等の契約テストを持つ。すべての失敗は機械可読な `code`、利用者向け `message`、`correlation_id` を含み、秘密情報を含めない。

Phase 1 の共通 error code は `scope_violation`、`scan_already_running`、`provider_unavailable`、`permission_denied`、`authentication_failed`、`rate_limited`、`timeout`、`malformed_data`、`storage_failure`、`cancelled`、`unsupported` とする。UI は未定義 code を安全に汎用エラーとして表示し、enum の追加でクラッシュしてはならない。

Phase 1 の必須 Command は以下とする。

```text
project.create / open / list
scope.validate / update
scan.preflight / start / cancel / get
entity.list / get
evidence.list
changes.list
unresolved.list / unresolved.disposition.set
override.create / revoke
diagram.get / layout.save
ipam.search
backup.create / restore.validate
```

- `scan.start` は immutable な Scope / Profile / Provider version snapshot を返す。Project ごとに `running` Session は一つだけとし、二重開始は `scan_already_running` で拒否する。
- `scan.cancel` の成功は「停止要求を記録した」ことを表し、終了を表さない。終了状態は `scan.get` またはイベントで確認する。
- `scan.*` event は `session_id`、単調増加 `sequence`、時刻、`correlation_id` を持つ。最低限 `started`、`provider_capability`、`progress`、`diagnostic`、`completed` を送る。
- UI はイベント欠落を前提とし、再表示時は `scan.get(session_id)` から状態を再取得する。イベントだけを永続的な真実として扱わない。
- `entity.list`、`ipam.search`、`changes.list`、`evidence.list` は cursor pagination、安定 sort key、filter schema を必須とする。大量データを IPC 一回で全件返してはならない。
- `unresolved.disposition.set` は `resolved` / `dismissed` / `deferred`、理由、対象 Entity、Evidence snapshot を必須とし、Audit Trail を append する。`unresolved.list` は未解決と再発を区別して返す。
- `diagram.layout.save` は Diagram ID と node position / collapse / filter だけを保存する。Network Entity、PhysicalLink、VLAN、IP、Evidence を変更できない。

### 15.3 永続化の最小制約

SQLite schema の詳細な DDL は migration で管理するが、次の一意性・参照規則は変更しない。

| 対象 | Phase 1 の制約 |
| --- | --- |
| Observation | `scan_session_id + sequence` は一意。追記専用であり、raw 値と parsed 値の参照を保持する。 |
| Evidence / Inference | Evidence は Observation を参照し、Inference は rule version と入力 Evidence の join record を持つ。JSON 配列だけで依存関係を保存しない。 |
| DeviceIdentity | identity type + canonical value の衝突を許容して conflict として保持する。DB の global unique 制約で別 Device を破棄しない。 |
| Interface | `device_id + ifIndex` は ifIndex が安定している期間の current projection key とし、ifIndex 再利用・名称変更は Evidence と時刻で履歴化する。 |
| MAC | normalized MAC は MAC Entity では一意とするが、Device / Interface / VLAN との関連は時系列 Observation として多対多を許容する。 |
| IPv4 / IPv6 | global/ULA は `network_context_id + binary_address`、link-local は `network_context_id + binary_address + link_scope_key` を current IP key とする。 |
| VLAN | `network_context_id + site_id + vlan_id` を current key とし、VLAN と Subnet の関係は別 Entity にする。 |
| Current State | Observation を上書きしない projection である。Session 終了時に一貫性を保つ transaction で更新し、途中状態を Diff baseline にしない。 |

Project DB と backup は IP、MAC、hostname、構成図を含む機微な運用データとして扱う。Phase 1 は OS のユーザー ACL で他ユーザーから読めない保存場所を既定とし、export/backup の保存先を利用者へ表示する。SQLite 暗号化、共有、クラウド同期は Phase 1 の前提にしない。

永続化する時刻はすべて UTC の RFC 3339 形式（小数秒あり）とし、比較・TTL・Diff は UTC で行う。UI は Project / Site の timezone を表示用にだけ用い、端末の locale 変更で Evidence の順序や期限を変えてはならない。

### 15.4 Provider と Discovery Profile の確定範囲

Provider は下表以外の情報を Phase 1 で暗黙に取得しない。未対応の MIB table / OS capability は diagnostic と `unknown_reason` で表す。

| Provider | Phase 1 で取得するもの | 実装上の制約 |
| --- | --- | --- |
| LocalNetwork | local NetworkAdapter（物理 / 仮想）、Interface、address、Route、ARP/NDP cache | passive のみ。OS が提供する bus / driver / firmware 等は Evidence 付きで取得する。管理者権限が必要な項目は capability で判定する。 |
| ICMP | 明示 Scope 内 target への 1 回の echo probe | broadcast / multicast / sweep の再送をしない。権限不足は `permission_denied`。 |
| TCP | 利用者が Profile で選択した port の connect 成否 | 既定 Profile の TCP port 集合は空。port は versioned profile data として表示・保存し、payload / banner は送受信しない。 |
| DNS | OS configured resolver による forward / reverse lookup | 外部 resolver を勝手に追加しない。TTL と応答種別を Evidence metadata に残す。 |
| SNMP v2c | SNMPv2-MIB の system、IF-MIB の logical Interface、BRIDGE/Q-BRIDGE-MIB の FDB/VLAN、LLDP-MIB の neighbor、IP-MIB / IP-FORWARD-MIB の address/route | UDP/161、GET/GETNEXT/GETBULK のみ。PCI / driver / firmware は取得対象外。各 table は独立に失敗可能で、walk 件数・bulk size・timeout を Profile で制限する。 |
| OUI | local versioned OUI database による prefix lookup | OUI vendor を Device vendor や確定 DeviceIdentity として扱わない。 |

`Quick` は LocalNetwork + DNS、`Standard` は Quick + ICMP、`Infrastructure` は Standard + SNMP / LLDP / FDB / Route、`Endpoint` は Standard + 利用者が選んだ TCP profile とする。いずれも Scope、host 上限、Provider Capability、Credential 解決後の Preflight を通らなければ開始できない。

### 15.5 正規化・状態遷移の実装規則

- MAC は uppercase 16 進・区切りなし、IP は binary 値、prefix は整数、hostname / VLAN / Interface 名は display 値と canonical lookup 値を分離する。canonical 化失敗は Entity を作らず `malformed_data` diagnostic とする。
- FDB は VLAN、bridge port、対応 ifIndex の対応付けが得られた場合にだけ VLAN-aware MACObservation を作る。対応付け不能な FDB は raw Observation と diagnostic を残し、PhysicalLink を生成しない。
- LLDP port ID / chassis ID は subtype と raw value を必ず保存する。ifName / ifAlias への解決に失敗した隣接は `unresolved_neighbor` として保持し、勝手に最も近い Interface を選ばない。
- `UserOverride` の Undo はレコード削除ではなく、元の優先状態へ戻す revoke event を append する。Merge/Split も同じく可逆な command event と Audit Trail を残す。
- Scan 中に検出した値の一部を UI に見せてもよいが、Current State / Diff は Session の status と projection transaction が完了するまで確定値として表示しない。

### 15.6 Phase 1 の追加受入条件

11. 正常終了しなかった `running` Session を Project 再 open 後に `interrupted` へ遷移させ、Current State と Diff baseline が不変であることをテストする。
12. IPC contract test で、Scan event の sequence 欠落後に `scan.get` から完全な状態を復元でき、二重 Scan start が拒否されることをテストする。
13. Credential、community、token、秘密鍵のテスト値が SQLite、JSON export、backup manifest、Application / Discovery / Diagnostic / Security-Audit log のいずれにも存在しないことを検査する。
14. SNMP fixture で IF-MIB、FDB/VLAN、LLDP、Route の一部 table が個別に失敗しても、取得済みの Observation と `unknown_reason` が保持され、誤った Link / Missing が生成されないことをテストする。
15. IPv4、IPv6 global/ULA、IPv6 link-local、MAC、hostname、LLDP subtype の canonical 化を property test と fixture で検証する。
16. Fresh DB 作成、前版 DB からの migration、migration 中断、backup restore、retention purge を検証し、旧 Observation が上書き・孤児参照にならないことをテストする。
17. Windows、macOS、Linux の各 PlatformNetworkProvider について、権限あり・権限なしの capability と fallback を少なくとも CI または管理済み手動試験で検証する。
18. backup restore で Secure Storage に存在しない Credential reference が `credential_required` になり、Project open / restore が自動 Scan や認証試行を発生させないことをテストする。
19. VLAN/SVI、bridge、bond/LAG 本体、tunnel、loopback、virtual NIC を含む fixture で、PhysicalLink が logical Interface に生成されず、LAG member のみを根拠に集約表示することをテストする。

### 15.7 運用・配布の最低条件

- 製品は、利用者が指定した DiscoveryScope と OS configured DNS resolver 以外へ、テレメトリ、利用状況、構成データ、Credential を送信しない。OUI database の更新、クラウド同期、auto-update は Phase 1 の機能に含めない。
- 各リリースは対応 OS と CPU architecture、アプリ version、schema version、migration の upgrade path を明記する。schema downgrade はサポートしないため、upgrade 前 backup と restore 手順を配布物に含める。
- Windows、macOS、Linux は Domain / Storage / Provider fixture / IPC contract を CI で検証する。ネイティブ package の署名、notarization、配布形式は出荷前の Release Checklist として別途確定し、未署名開発 build を本番配布物と混同しない。
- OSS の OUI DB、MIB、Rust/TypeScript dependency は version とライセンスを台帳化する。SBOM と既知脆弱性スキャンは最初の配布候補の release gate とする。

### 15.8 実装開始判定

Phase 1 は本仕様 v0.4.2 により実装を開始してよい。実装順序は次とする。

```text
1. Domain 型、SQLite migration、Repository、SecretStore abstraction
2. Scope / Profile / Preflight、ScanSession、Provider capability、IPC contract
3. LocalNetwork / DNS / ICMP / SNMP Provider と Observation fixture
4. Evidence、Entity Resolver、Current State、Topology / IPAM projection、Diff
5. Physical / L2 / L3 / IPAM / Evidence / Unresolved UI
6. 再スキャン、backup/restore、crash recovery、性能・クロスプラットフォーム試験
```

Phase 2/3 の実装、ベンダー固有ドライバ、SSH/CLI、クラウド API、設定変更機能は、Phase 1 の受入条件を全て満たすまで開始しない。

## 16. 画面設計

Phase 1 の画面・遷移・表示状態・操作制約は [screen-design-v1.md](screen-design-v1.md) を正とする。特に、Topology の確度表現、Device Inventory / 詳細 Drawer、Evidence Inspector、IPAM の未使用判定、Scan Preflight、Changes の coverage 抑制、Unresolved の解決操作は、同文書の受入条件を満たさなければならない。

## 17. NIC 追加後の独立レビューで採用した補完

| 優先度 | 採用した補完 | 反映先 |
| --- | --- | --- |
| P0 | Unresolved の一覧・解決・却下・保留を IPC と Audit Trail で明示 | 15.2、screen-design 6.2 |
| P0 | 論理専用 Interface に PhysicalLink を生成しない | 5.1.1、15.6 条件 19 |
| P1 | restore 後の secret missing を検出し、自動 Scan / 認証試行を防止 | 8、15.6 条件 18 |
| P1 | Canvas の node 移動は Layout 保存だけであり、Link を変更しない | 15.2、screen-design 4.1 |
| P2 | virtual NIC の未取得属性を `not_observed` と表示 | 5.1.1、screen-design 4.4 |

## 18. DB設計

Phase 1 の SQLite schema、table/column、FK、一意制約、projection 更新、retention、backup、migration、DB 受入条件は [database-design-v1.md](database-design-v1.md) を正とする。Provider は同文書の `observations` / `provider_diagnostics` 以外へ直接書込みを行わず、Entity と Current State は Evidence を経由して更新する。

## 19. UniFi風 UI 追加後の独立レビューで採用した補完

| 優先度 | 採用した補完 | 反映先 |
| --- | --- | --- |
| P0 | Phase 1 の回線表示を単一 Scan 時点に限定し、traffic / utilization chart を除外 | screen-design 4.5 |
| P0 | Find in topology が filter / collapse 内の対象を一時 reveal する | screen-design 4.6 |
| P1 | Device Inventory の Connection を事前計算 projection として保存 | database-design 8–10 |
| P1 | リモート Wi-Fi Client の RSSI / SNR は Controller Provider 導入まで `not_observed` | screen-design 4.5 |
| P2 | Drawer の IP から同一 Context の IPAM Inspector へ遷移 | screen-design 4.5 |

## 20. 実装進捗（2026-09-04）

この節は実装済み範囲を示すものであり、未実装機能を仕様上の完了とみなしてはならない。

| 項目 | 状態 | 実装上の境界 |
| --- | --- | --- |
| Tauri / React / Rust / SQLite の骨格 | 実装・Windows 検証済み | `cargo test`、`npm run build`、`tauri build --no-bundle` は Windows で成功。macOS / Linux の native build は未検証。 |
| Scope、read-only Profile、Scan state | 実装済み | Scope 形式検証は Preflight 画面から Tauri IPC へ接続済み。active probe を有効化できず、Provider 未実装の scan 要求は成功扱いにせず拒否する。 |
| NIC / Interface と PhysicalLink | 実装済み | physical port evidence を持つ物理 Interface 同士以外は、Domain validation と SQLite trigger の両方で拒否する。 |
| SQLite foundation migration / Inventory projection | 実装済み | 現在は foundation schema と空 projection。Observation からの resolver / projection 更新は未実装。 |
| 画面 shell、Inventory、Drawer、Topology、IPAM、Preflight | 実装済み | `read-only`、`provider 未実装`、`Unknown` を表示し、traffic chart や擬似的な探索結果を表示しない。 |
| LocalNetwork capability | 実装済み | OS API で local Interface/address を受動列挙。adapter/physical state は unknown/not_observed、route/ARP/NDP/ICMP/raw packet は unsupported。 |
| Project / Scope 初期化 | 実装済み（foundation） | 利用者が入力した Project 名と明示した Scope を一件だけ保存する。Scope ごとの include/exclude、max_hosts/max_depth、approved_at、scope_targets は次の migration で追加し、未指定値を補完してはならない。 |
| Discovery scan Provider、SecureStore、resolver、Diff、backup/restore | 未実装 | これらを実装・試験するまで、実ネットワークに対するスキャン機能を出荷しない。 |
