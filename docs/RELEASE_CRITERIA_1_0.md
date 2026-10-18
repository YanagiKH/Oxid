# Oxid v1.0 發布驗收矩陣（R0 草案）

> **狀態：方向已採用（使用者已採用 ROADMAP_V2）。本矩陣是 R0 草案，待審查；尚未定案的只有三類：ROADMAP_V2 標為建議的數字（本文標 proposed）、維護與驗收資源（第 5 節）、以及本矩陣本身的整理細節。目前判定：v1.0 未達；G1–G10 全部未達。**
>
> 本文件把 [ROADMAP_V2.md](ROADMAP_V2.md)（使用者提供的原文，逐 byte 保留）第三、十四、十五節整理成導航用的驗收矩陣摘要。它不是發布認證，也不改變既有 [ROADMAP.md](ROADMAP.md)、[feature-status.json](feature-status.json)、CI workflow 或任何 pin。
>
> **本文件只是導航摘要，不取代、也不弱化原文的完整驗收條件**；規範優先順序與缺項處理見第 0 節。

| 項目 | 值 |
|---|---|
| 現況檢查基準 | `main` `597c323118ecbd103c5761b9e4e5a976c9f2fb11`（2026-10-18） |
| 最新公開 release | v0.9.0（2026-08-27，依 ROADMAP_V2 記載）；`main` 之後的能力不算已發布 |
| 原文身分 | `docs/ROADMAP_V2.md`，35028 bytes，sha256 `09c787fd6ffe72db1d5aa1df9c49214b3e77e760d2689fe8e5d04405fa429b09` |
| 本切片驗證 | 只讀原始碼、規格與清冊；沒有 build、test、效能或平台實測 |

## 0. 規範優先順序與缺項回指

本文件是 ROADMAP_V2 的導航摘要，用來快速找到各關卡的證據與缺口，**不是**較精簡的替代 checklist。判定任何關卡時依下列優先順序：

1. **[ROADMAP_V2.md](ROADMAP_V2.md) 原文**是唯一規範來源，包括第三節 G1–G10 的完成條件與「不接受的替代證據」、各工程主線（R0–R9）的工作清單、完成條件與驗收專案、第十四節六個正式專案與共同要求、第十五節 15.1／15.2／15.3、第十八節完成條件。
2. **本文件**只做摘要與現況對照。措辭與原文不一致、比原文寬鬆或漏列時，一律以原文為準，並應修正本文件；不得以本文件的較短寫法降低門檻。

**缺項回指**：本文件沒有逐條列出的原文要求，**不代表豁免、延後或已滿足**。判定任一關卡通過前，必須回到原文對應章節逐項核對；矩陣的「必要證據」欄是索引，不是完整清單。

以下原文條件在矩陣中最容易被摘要掉，特別點名，全部仍為必要條件：

- **§15.2 可靠性與安全（全部六項）**：穩定範圍沒有已知阻斷性的錯誤編譯、soundness、資料損毀或重大安全問題；支援功能有正向、compile-fail、邊界、組合、fuzz 與永久回歸測試；compiler crash 有保留證據與縮減輸入的流程；unsafe、FFI、allocator、並行 runtime 與 artifact reader 經專門審查；支援平台執行適用的 sanitizer／動態檢查；「沒有已知缺陷」不寫成「證明不存在任何缺陷」。
- **§15.1 效能的非數字條件**：效能比較前，輸出、錯誤處理與安全語意必須一致（等價正確性）；短測試保存有效樣本（數量門檻 proposed：至少 30 個）、環境、原始數據與信賴區間；AI 另測梯度、收斂、記憶體與 time-to-quality，不沿用純 CPU loop 門檻；跨平台能力與跨平台效能宣告分開；不得靠默默移除已承諾的 bounds／overflow／ownership 規則取得優勢。比值數字本身是建議預算（proposed），但上述條件不因數字未定而放寬。
- **§15.3 穩定性與維護**、**第十四節共同要求**、**第三節「不接受的替代證據」**：矩陣只摘錄部分，判定時以原文全文為準。

## 1. v1.0 發布範圍

v1.0 的發布條件是下列全部成立（AND 關係，不能以某項超額抵銷另一項）：

- **穩定語言路徑**：typed 語意以 stable edition 承諾，並同時提供 **native 編譯**與**直譯執行**兩條路徑，兩者對已承諾語意一致。
- **五個 native 平台**：Linux x86_64、Linux ARM64、Windows x86_64、macOS x86_64、macOS ARM64。
- **六個正式專案**：原生 CLI／資料處理工具、HTTPS／資料庫服務、背景工作程序、跨語言原生函式庫、原生 CPU 訓練應用、Oxid compiler／工具鏈；與 compiler fixtures 分開維護。
- **CPU 訓練**：原生 CPU 的 tensor、autograd、optimizer、資料流程、收斂與 checkpoint resume（ROADMAP_V2 R9-A）。
- **自宿主**：指定 compiler 與標準庫由公開 Oxid 種子重建，C2/C3 固定點，無隱藏 Rust frontend fallback。
- **兩個 RC**：連續兩個 release candidate 通過全部必要關卡；核心變更後重跑受影響驗收。

**不屬於 v1.0（留給 v1.x 或成熟度認證，不混算）**：GPU 訓練（R9-B）、分散式／多節點訓練與成熟 AI 平台（R9-C）、完整 Rust crate／procedural macro 相容、完整 Python 動態語意、WASM／嵌入式／桌面行動／遊戲／系統等領域認證（ROADMAP_V2 第十六節）。

## 2. 證據分類

每筆證據只能屬於一類，不得互相折算：

| 類別 | 定義 | 能證明什麼 |
|---|---|---|
| 歷史 | 綁定較舊 commit 的紀錄、已歸檔報告、過去 CI | 當時、該範圍曾成立；不代表目前或發布版本 |
| current | 綁定目前檢查基準 commit 的本地或 CI 結果 | 目前 `main` 在宣告範圍與平台上成立；不代表已發布 |
| release | 綁定 RC／正式 tag 與發布 artifact 摘要的結果 | 該發布版本在宣告平台上成立 |

`fixture_only`、experimental provider、legacy demo 與 bounded component 結果，最多只能作為對應關卡的開發佐證，不能單獨判定任何關卡通過。

## 3. 摘要矩陣

| 關卡 | 判定 | 下一個交付（建議） | 長期 owner／reviewer |
|---|---|---|---|
| G1 語言穩定 | 未達 | v1 typed 範圍清單＋相容性／遷移政策草案 | 未指派（release blocker） |
| G2 安全核心 | 未達 | 一般字串與一個可成長容器的 ownership／析構契約 | 未指派（release blocker） |
| G3 正式原生編譯 | 未達 | 分離 bounded 與 hosted policy；debug/release profile；第二個 native target | 未指派（release blocker） |
| G4 完整 Runtime | 未達 | typed-native 二進位檔案 I/O 與程序／結束碼契約 | 未指派（release blocker） |
| G5 套件與建置 | 未達 | R1：typed manifest/edition 接入單一 compiler session 的端到端切片 | 未指派（release blocker） |
| G6 開發工具 | 未達 | typed `build/test` 與 formatter 的同一專案設定流程 | 未指派（release blocker） |
| G7 互通與遷移 | 未達 | C ABI 匯出一個 Oxid 函式庫的最小真實案例 | 未指派（release blocker） |
| G8 真正自宿主 | 未達 | 列出 compiler 實作所需語言能力的缺口清單 | 未指派（release blocker） |
| G9 正式專案與效能 | 未達 | 凍結六個專案的範圍與效能 corpus 定義 | 未指派（release blocker） |
| G10 發布與維護 | 未達 | 簽章、回退與支援週期政策草案 | 未指派（release blocker） |

近程執行分工（AI 協作者，僅限這兩條主線，不等於上表的長期維護責任）：

- **R0 發布契約文件**：實作 fufu，審查 kertyopuiop-miku。
- **R1 typed project 完整流程**：雲端 agent 規劃，kertyopuiop-miku 整合，fufu 之後做獨立審查。

## 4. 逐條驗收

「現況缺口」只引用基準 commit 中可查的檔案；未在本切片核對的項目明寫未核對。

### G1：語言穩定

- **使用者成果**：開發者依 v1 規格寫的程式，在相容版本升級後語意不變；破壞性變更只經 edition 或明確 major policy。
- **必要證據**：v1 規格與支援矩陣、相容性／棄用政策、遷移規則、對應規格條目的 conformance tests（current＋release）。
- **現況缺口**：預設 edition 仍是 `legacy-0.9`；typed-preview 是 opt-in experimental（[spec/typed-preview.md](../spec/typed-preview.md)）。`feature-status.json` 33 筆中 experimental 28、proposed 3、implemented 2，沒有 validated 或 stable。沒有 v1 規格、相容性政策或 conformance suite。
- **下一個交付**：v1 typed 範圍清單（納入／延後／不支援）與相容性政策草案。

### G2：安全核心

- **使用者成果**：不退回 legacy 或外部語言，就能安全寫出可成長資料結構、回傳資料的函式、closure 與多模組函式庫。
- **必要證據**：堆積配置、一般容器、跨 scope 借用、析構順序、並行安全契約的正向與 compile-fail 測試；無已知阻斷性 soundness 缺陷的追蹤紀錄；以及 §15.2 全部可靠性與安全條件（見第 0 節，本行不是完整清單）。
- **現況缺口**：目前為受限能力（owned record、固定陣列、call-only borrowing、slice、部分 enum／match）；一般堆積配置、容器、析構、可儲存／回傳的參考、完整生命週期與並行安全尚未提供。
- **下一個交付**：一般字串與一個可成長容器的 ownership／析構契約與端到端使用案例。

### G3：正式原生編譯

- **使用者成果**：在五個平台建置、執行、除錯 release 品質的 native 程式；長時間程式不因示範級 fuel 終止。
- **必要證據**：五平台 native 建置／執行／除錯（release 證據）；debug/release profile 與最佳化正確性測試；十萬行有效多模組專案的 clean／incremental build（門檻為 proposed）。
- **現況缺口**：`src/frontend/native.rs` 固定 LLVM `19.1.7`、`x86_64-unknown-linux-gnu`、`-O0`；`src/frontend/oir/execute.rs` 的 `MAX_FUEL` 為 1,000,000；native preview 拒絕遞迴（ROADMAP_V2 第七節）。CI 一般矩陣是 Linux x86_64、Windows x86_64、macOS x86_64、macOS arm64 四個 host，沒有 Linux ARM64；這不是五平台 native 證據。
- **下一個交付**：保留 bounded profile，另立 hosted execution policy；加入 debug/release profile 與第二個 native target。

### G4：完整 Runtime

- **使用者成果**：在 typed-native 路徑完成檔案、二進位串流、程序、執行緒與非同步 I/O，且取消與資源釋放可預期。
- **必要證據**：各資源的正式契約與正常完成／提早返回／取消／配置失敗／部分初始化測試；48 小時服務負載測試（proposed）。
- **現況缺口**：檔案、程序、TCP/HTTP、JSON 與 lazy task 只存在於 legacy 0.9 路徑（[ROADMAP.md](ROADMAP.md)）；typed-native 沒有同等能力，也沒有真正的執行緒、executor 或取消契約。
- **下一個交付**：typed-native 二進位檔案 I/O 與程序參數／結束碼契約。

### G5：套件與建置

- **使用者成果**：`new → build → run → test → package` 用同一份專案設定；可鎖定、離線重建、升級並拒絕被竄改的來源。
- **必要證據**：R1 驗收專案（fixture 目錄外的多套件應用走完整流程）；workspace、多版本依賴、registry、locked/offline/vendor、供應鏈驗證測試。
- **現況缺口**：spec 明載「Manifest edition propagation and typed project builds are not implemented」。已有 bounded typed-project public dispatch（Unit4 ledger），但不是正式 typed project build。legacy 有本地與 commit-pinned Git 依賴、version 1 lockfile 及 RFC 0029 SHA-256 完整性；沒有 registry、同名多版本、發布者簽章。
- **下一個交付**：R1 第一個端到端切片（由雲端規劃，不在本文件範圍內實作）。

### G6：開發工具

- **使用者成果**：用一條官方流程完成安裝、編輯、格式化、測試、除錯、文件與效能分析。
- **必要證據**：LSP、formatter、test runner、debugger、文件工具、profiler 支援日常流程；至少 5 位非核心開發者、每項任務 4/5 人無私下指導完成（proposed）。
- **現況缺口**：有 typed formatter 驗證（CI 執行 `scripts/verify_typed_formatter.py`）與 `editors/vscode` 的 tasks 設定；本切片未找到 LSP 或 debugger 整合，各工具成熟度未逐項核對。外部使用者驗收尚未進行。
- **下一個交付**：typed `build/test/fmt` 共用同一專案設定的流程（依附 R1）。

### G7：互通與遷移

- **使用者成果**：同一份 Oxid 原生函式庫可被 C、Rust、Python 專案使用。
- **必要證據**：三方整合的錯誤、回呼、跨執行緒與資源釋放測試；轉換工具明列不支援範圍。
- **現況缺口**：0.9 提供跨語言 process adapter（subprocess 類型，屬不接受的替代證據）；本切片未找到 C ABI 函式庫匯出或 Python 原生擴充建置的驗收案例。
- **下一個交付**：C ABI 匯出一個 Oxid 函式庫並由 C 呼叫的最小真實案例。

### G8：真正自宿主

- **使用者成果**：拿到公開 Oxid 種子與原始碼，無 Rust／Cargo 即可重建指定版本。
- **必要證據**：正式 Oxid compiler 編譯完整 compiler source 與其他合法程式；C1/C2/C3 重建與同 target C2/C3 比對；無 Rust 重建紀錄（release 證據）。
- **現況缺口**：預設 compiler 路徑仍由 Rust 控制；Oxid lexer／parser／static frontend 是 bounded component，lexical provider 是真實接點但 parser、semantic、ownership、OIR、backend 仍由 Rust 掌握。C1/C2/C3 固定點與無 Rust 重建列為 [open gates](SELF_HOSTING.md#required-future-self-hosting-evidence)。
- **下一個交付**：compiler 實作所需語言能力（字串、動態容器、檔案、錯誤處理）的缺口清單，對應 G2／G4。

### G9：正式專案與效能

- **使用者成果**：六個正式專案由一般使用者完成開發、部署與升級；CPU 訓練案例收斂並可恢復。
- **必要證據**：六專案走一般 `build/test/package`；至少三個有非 compiler 核心作者參與維護、一輪跨版本升級、必要應用 30 天試用（數字皆 proposed）；固定 corpus 效能預算（數字 proposed：對 Rust 幾何平均 ≤ 1.25、單項 ≤ 2 倍、峰值 RSS ≤ 1.5 倍）；比較前的等價正確性，以及短測試保存有效樣本（數量門檻 proposed：至少 30 個）、環境、原始數據與信賴區間；AI 另測梯度、收斂、記憶體與 time-to-quality；MLP 與一個較複雜模型的原生 CPU 訓練。完整條件以原文 §15.1 與第十四節為準（見第 0 節）。
- **現況缺口**：六個專案尚未建立；效能 corpus 未凍結；[benchmark harness](BENCHMARKS.md) 只量測 Oxid 本身，不是對 Rust 的比較；沒有 typed buffer、tensor、autograd 或原生訓練實作。
- **下一個交付**：凍結六專案的範圍描述與效能 corpus 定義，之後不得因結果不利移除。

### G10：發布與維護

- **使用者成果**：安裝指定版本、驗證來源、更新、回退與離線部署；知道支援期限與安全回報管道。
- **必要證據**：artifact 來源、摘要、簽章、平台與外部依賴清單；更新／回退演練；版本化文件；有實際負責人的安全維護承諾（proposed：v1 穩定系列至少 24 個月）。
- **現況缺口**：`.github/workflows/release.yml` 為四個 host 打包並附 `.sha256`，沒有簽章或 provenance；[SECURITY.md](../SECURITY.md) 只承諾 `main` 與最新 release line，沒有支援週期；沒有更新／回退流程與實際維護責任人。
- **下一個交付**：簽章、回退與支援週期政策草案（數字保留 proposed）。

## 5. 尚未落實的人力與資源（release blocker）

下列條件需要真人或硬體，目前沒有落實，**每一項都是 release blocker**；本文件不填人名、不代為承諾：

- G1–G10 各關卡的長期 owner 與 reviewer（現為未指派）。
- 安全維護負責人與支援週期資源（24 個月為 proposed）。
- 發布簽章金鑰的保管、輪替與撤銷負責人。
- 至少 5 位非核心開發者的使用者驗收（proposed）。
- 至少三個正式專案的非 compiler 核心維護者，與 30 天實際使用（proposed）。
- Linux ARM64 原生驗證環境，以及五平台的 native 除錯／release 驗證資源。
- 48 小時負載測試與效能量測用的固定基準機器（proposed）。

## 6. 接入既有清冊的最小建議（待審查，本切片未實作）

1. `docs/feature-status.json`：每筆加 `release_gates`（G1–G10 代號），並把 `status` 擴成 ROADMAP_V2 R0 要求的四階段（有實作／已接入正式路徑／指定平台已驗證／已發布）；`evidence` 分成歷史／current／release。`baseline_commit` 目前是 `7e681d25…`，與 597c3231 不同，更新前需重核各筆。同步調整 `scripts/verify_feature_status.py` 驗證新欄位。
2. `docs/ROADMAP.md`：加一段連到 ROADMAP_V2 與本文件，說明 M0 對應 R0、M1/M5/M7 對應 R1；保留既有 M0/M1/M8 清冊。
3. `.gitattributes`：加 `docs/ROADMAP_V2.md -text`。原文為 CRLF 且無結尾換行，避免 Windows `core.autocrlf` 等設定改動已承諾的 bytes。
4. 本文件第 3 節的「判定」日後由 release gate runner 依必要工作結果產生（ROADMAP_V2 第十七節優先序 8），不另造管理框架。
