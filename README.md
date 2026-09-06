# Network Analyzer

ローカル専用のネットワーク可視化・IPAM アプリケーションです。現在は Phase 1 foundation を実装しています。

## 実装済み

- Tauri 2 / React / TypeScript / Rust / SQLite のプロジェクト構成
- Scope、読み取り専用 Profile、Scan lifecycle、Evidence、NIC / Interface のドメイン契約
- VLAN / SVI / bridge / bond-LAG / tunnel / loopback / virtual NIC を PhysicalLink endpoint にできない検証
- SQLite migration、foreign key、単一 running scan 制約、PhysicalLink DB trigger、空の Inventory projection
- Discovery provider 未実装の scan 要求を必ず拒否する IPC と、read-only / Unknown を明示する最小 UI
- Preflight 画面から Tauri IPC へ接続した CIDR / 単一 IP / Seed / Context の Scope 形式検証
- LocalNetwork capability（OS API による受動的な NIC / Interface / address 列挙）と、unsupported の route / ARP/NDP / ICMP / raw packet 表示。OS API で根拠が得られない物理・仮想種別は `unknown` のまま表示

## 未実装

Discovery scan provider、OS secure store、実ネットワーク探索、Evidence resolver、IPAM / topology projection の生成、backup / restore は未実装です。LocalNetwork はローカル Interface/address の受動列挙だけを提供し、未実装 provider を成功した scan として扱いません。

## セットアップと検証

Windows のこの開発環境では Rust stable と Node.js により、`cargo test`、`npm run build`、`tauri build --no-bundle` を確認済みです。別 OS では、Rust stable と Node.js 20 以降を導入後、以下を実行します。

```powershell
npm install
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri dev
```

資格情報の実値は DB、IPC、ログ、画面に保持しません。
