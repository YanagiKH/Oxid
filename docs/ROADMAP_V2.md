**建議將 Oxid v1.0 定義為「能讓開發者長期維護正式專案的第一個穩定版本」，而不是「完成某個比例的編譯器功能」。**

這次路線圖的主軸應改成：

> **把現有的 typed-preview、ownership 驗證與 LLVM 原生編譯，整合成一套能完成日常開發、套件管理、除錯、部署、升級與維護的正式語言產品；再沿同一套穩定核心擴展到 AI、嵌入式與其他專業領域。**

## 一、目前進度的重新判定

本次檢查基準為 **2026-10-19 可取得的 `main`，commit `597c323118ecbd103c5761b9e4e5a976c9f2fb11`**，提交日期為 2026-10-18；對應 Repository CI 第 372 次執行成功。目前最新公開 release 仍是 **2026-08-27 發布的 v0.9.0**，不能把後續 `main` 的能力視為既有 release 已提供的功能。  

先前的 `.md` 附件已失效，本次依目前倉庫及上輪對話中的路線圖重訂。檢查包含遠端原始碼抽查、規格、功能清冊與 GitHub CI；**沒有在本環境重新完成編譯、效能或 GPU 實測，因此不是完整安全認證。**

### 1. 已有進展與真正剩餘的差距

| 面向 | 目前確認的進度 | 距離正式主力開發語言的差距 |
|---|---|---|
| 靜態前端 | 已有獨立的 typed-preview、HIR、OIR、控制流程及檢查路徑。 | 尚未形成可穩定承諾的一般用途語言規格。 |
| Ownership | 已有 owned record、固定陣列、call-only borrowing、slice、部分 enum／match 等受限能力。 | 一般堆積配置、容器、析構、可儲存及回傳的參考、完整生命週期仍需補齊。 |
| 原生編譯 | 確實產生 Linux x86_64 ELF，不再只是 AST 封裝。 | `native.rs` 仍固定 LLVM 19.1.7、`-O0` 與單一 target；尚非跨平台正式編譯器。 |
| 專案工作流程 | 已有 typed 模組與宣告解析。 | typed-preview 規格仍明列：manifest edition propagation 與 typed project build 尚未實作。 |
| Oxid 自寫前端 | 已有 Oxid lexer、parser、static frontend 元件及實際 lexical provider 接點。 | 正式解析、語意分析、ownership、OIR 與 backend 權威仍由 Rust 路徑掌握。 |
| Runtime／套件 | legacy 路徑已有腳本、JSON、檔案、網路及 path／Git 依賴。 | 不等於 typed-native 路徑已具備相同能力；並行 runtime、正式套件生態及一般二進位 I/O 仍有缺口。 |

這些判定來自目前的 typed/native 規格、實際 compiler driver 與 native tool boundary，而不是只根據 README 功能名稱。     

另外，最新合併的 PR #71 明確把新增能力標為 `fixture_only=True`、`qualified=False`；它是 checked-caller 邊界的開發驗證，不是 production V3 frontend 已完成。這項工作有價值，但不能直接折算成正式語言功能完成度。

**因此，目前沒有足夠證據核定「整體成熟度已完成 50%」。**更準確的判定是：

> **Oxid 已建立值得保留的靜態核心與驗證基礎，但接下來的關鍵是產品整合、一般用途能力與真實專案驗收。**

目前倉庫自己的路線圖也明確記載 M0、M1、M8 尚未完成。新版應保留已有成果，但不再讓「新增一個受限元件」與「完成一項產品能力」使用相同的完成標準。

---

# Oxid：從目前進度到 v1.0 與成熟專業語言的工程路線圖

以下是**新的工程與發布規格提案**。其中的數字是建議採用的驗收門檻，不是目前專案已達成的測量結果。

## 二、先定案：Oxid v1.0 到底代表什麼

### 2.1 v1.0 的正式定義

> **Oxid v1.0 是第一個具有穩定語意、靜態安全核心、跨平台原生編譯、正式套件工作流程、完整開發工具與可維護部署能力的版本。**
>
> **開發者能不依赖未揭露的 Rust／Python 執行路徑，使用 Oxid 完成一般 CLI、後端服務、背景程序、原生函式庫及基本數值／CPU 訓練專案，並持續升級維護。**

「成熟」也必須包含穩定性承諾。Rust 以 edition 管理不相容語言變更，Python 則以公開相容性與棄用政策管理既有使用者的升級風險；這些都應成為 Oxid 的產品要求，而非等功能夠多之後才補。[Rust 文檔](https://doc.rust-lang.org/edition-guide/editions/)

### 2.2 v1.0 必須承擔的範圍

| 範圍 | v1.0 必須交付 |
|---|---|
| 一般原生開發 | CLI、檔案處理、程序整合、長時間背景程序、原生函式庫。 |
| 後端服務 | 真正並行的 I/O、HTTPS／JSON、資料庫整合、可觀測性與正常關閉。 |
| 開發體驗 | 一致的建立、建置、測試、格式化、除錯、文件及發布流程。 |
| 生態互通 | C ABI、可驗證的 Rust 互通、Python 原生擴充整合。 |
| 數值與 AI 基礎 | typed buffer、CPU tensor、autograd、optimizer、基本訓練與 checkpoint。 |
| 工具鏈自主 | 指定編譯器與標準庫可由 Oxid 種子工具鏈重建，沒有隱藏 Rust frontend fallback。 |

### 2.3 不應綁死 v1.0 的要求

下列工作保留為正式長期目標，但**不是 v1.0 的必要前置条件**：

- 完整無修改編譯所有 Rust crate、所有 Rust edition 與所有 procedural macro。
- 完整模仿 Python 動態語意，或直接支援全部 Python 套件。
- 自行重寫 LLVM、linker、所有密碼學函式庫及所有 GPU vendor library。
- 在每個 workload 上都比 Rust 快。
- 同時完成大型 GPU 預訓練、所有嵌入式平台、作業系統核心與完整 GUI 生態。

**取代 Rust 作為主要開發語言，不等於先成為第二套 rustc；接近 Python 的使用便利性，也不等於複製 CPython。**

---

## 三、v1.0 發布的十個必要關卡

十個關卡是 **AND 關係**，不能用某一項超額完成抵銷另一項缺失。

| 關卡 | 完成條件 | 不接受的替代證據 |
|---|---|---|
| **G1：語言穩定** | v1 規格、支援矩陣、相容性政策、遷移規則與 conformance tests 完整對應。 | 只有 RFC、parser 接受語法或示範程式。 |
| **G2：安全核心** | 一般容器、堆積配置、借用、析構與並行安全契約可用；沒有已知阻斷性 soundness 缺陷。 | 僅 call-only borrowing 或固定大小資料結構。 |
| **G3：正式原生編譯** | 指定五個平台具原生建置、執行、除錯與 release profile；一般專案不受示範級固定 fuel 限制。 | 四平台 interpreter CI 加上一個平台的 native preview。 |
| **G4：完整 Runtime** | 檔案、二進位 I/O、程序、執行緒、非同步 I/O、取消與資源釋放均有正式契約。 | lazy task 或循序 `join_all`。 |
| **G5：套件與建置** | typed project、workspace、鎖檔、多版本依賴、registry、離線建置與供應鏈驗證可用。 | 只成功下載 Git repository。 |
| **G6：開發工具** | LSP、formatter、test runner、debugger、文件及效能分析能支援完整日常流程。 | 工具命令存在，但只輸出 preview。 |
| **G7：互通與遷移** | C／Rust／Python 整合有真實使用案例，轉換工具明確呈現不支援範圍。 | 只有 subprocess wrapper。 |
| **G8：真正自宿主** | 正式 Oxid compiler 重建、C2/C3 固定點、無 Rust 重建流程成立。 | artifact encode/decode 相等。 |
| **G9：正式專案與效能** | 固定的一組真實專案、壓力測試、恢復測試、效能及使用者工作流程達標。 | 只計算 unit test 數量。 |
| **G10：發布與維護** | 簽章、更新、回退、安全回報、支援週期、責任人與版本化文件已運作。 | 上傳 release 壓縮檔。 |

**G9 包含基本原生 CPU 訓練驗收；GPU 與分散式訓練另外認證，不混算。**

---

## 四、工程主線 R0：收斂現況與發布契約

**對應舊路線圖：M0。這一階段不是重做全部基礎，而是讓已有證據能真正決定發布。**

### 工作清單

- [ ] 建立單一 v1.0 發布規格，逐條對應 G1–G10。
- [ ] 擴充現有 `docs/feature-status.json`，分開記錄「有實作」「已接入正式路徑」「指定平台已驗證」「已發布」。
- [ ] 將歷史證據、目前 commit 證據、release 證據分開，避免舊成功紀錄被誤認為目前版本已驗證。
- [ ] 每個 release blocker 指派實際 owner 與 reviewer，不允許發布關卡仍為 `unassigned`。
- [ ] 把 `fixture_only`、experimental provider、legacy demo 與正式 compiler capability 明確分類。
- [ ] 保留 existing tests 與負向驗證，不為了簡化路線圖刪掉已建立的安全邊界。
- [ ] 凍結第一批正式專案及效能 corpus，之後不得因結果不利而臨時移除。
- [ ] 每項功能除了證明「非法輸入被拒絕」，也必須證明「正常使用者任務能完成」。

目前 feature inventory 仍標示為非完整 M0 清冊，且已讀項目存在未指派的 owner／reviewer，這部分可直接從既有檔案補齊，不需要另起不相容的管理系統。

### 完成條件

任何人只讀發布規格與證據索引，就能回答：

**這個版本還差什麼、由誰處理、用什麼測試判定、哪個 commit 證明完成。**

---

## 五、工程主線 R1：先打通正式專案工作流程

**對應舊路線圖：M1、M5、M7 的共同整合部分。**

目前最直接的產品缺口是：typed-preview 雖然已有 `check/run/compile`，但還沒有正式 typed project build 與 manifest edition 傳遞。這會使使用者每天都必須知道內部 preview 路由與特殊參數。

### 工作清單

- [ ] 將 manifest、workspace、edition、target、profile 與 dependency graph 接入同一個 compiler session。
- [ ] 让 `new → build → run → test → package` 使用同一份專案設定，而不是各自選擇不同語意路徑。
- [ ] 新專案明確記錄穩定 edition；舊專案保留 legacy 遷移路徑，不以語法猜測自動改變語意。
- [ ] 分離語言 edition、最佳化 profile、backend、target 與 execution policy。
- [ ] 明確定義 stdout、stderr、exit status、JSON diagnostics、輸出覆寫及建置失敗後的產物保留規則。
- [ ] 一般 `build` 可安全更新其管理的建置產物；低階 `compile --output` 保留明確的覆寫契約。
- [ ] 完成可重現 clean build、增量 build、平行 build 與 cache invalidation。
- [ ] `doctor` 檢查真正的 compiler、SDK、linker、target library 與套件環境，而不只是檔案存在。
- [ ] 一般 Oxid 使用者不必手動配置未揭露的 Rust／Python bootstrap 程序；系統 SDK 等必要外部條件清楚列出。

### 驗收專案

建立一個**不在 compiler fixture 目錄內**的多套件應用，使用一般發布工具鏈完成：

> 建立專案 → 引入依賴 → 修改一個模組 → 增量建置 → 執行測試 → 產出 native executable → 在乾淨部署環境執行。

**這條流程沒有打通之前，不應繼續以新增更多 frontend 小範例當成主要進度。**

---

## 六、工程主線 R2：把受限安全子集擴展成一般用途語言

**對應舊路線圖：M1、M2。**

### 6.1 型別與語言表達能力

- [ ] 完整整數型別、指標寬度整數、`f32/f64`、字元、UTF-8 字串與二進位資料型別。
- [ ] 統一 overflow、除法、轉型、浮點數、相等比較與求值順序；debug/release 不得偷偷改變已承諾的安全語意。
- [ ] 一般 `struct`、具組合 payload 的 `enum`、tuple、array、slice、pattern matching 與巢狀資料結構。
- [ ] 泛型函式／型別、trait bounds、associated types 與足以支援標準集合及 iterator 的抽象能力。
- [ ] closure、函式值、遞迴、一般 `for`／iterator，以及正常的錯誤傳遞。
- [ ] 公開 API 有清楚型別契約；內部程式允許型別推導，降低不必要的語法負擔。
- [ ] 簡潔語法與傳統語法使用同一套語意，不發展兩套難以互通的語言。

### 6.2 Ownership 與資源管理

- [ ] 從 call-only borrowing 擴展到跨 scope 借用、合法的參考回傳及必要的參考型別組合。
- [ ] 完成堆積配置、`Box` 類型、動態序列、字串及 map 所需的生命週期管理。
- [ ] 完成 partial move、重新初始化、析構順序、錯誤路徑 cleanup 與 closure capture。
- [ ] 定義共享所有權、interior mutability、跨執行緒傳遞及同步規則。
- [ ] 明確設計 `unsafe`、FFI ownership、原始指標、對齊、初始化與 pointer provenance。
- [ ] 固定 panic 行為與 FFI 邊界；不得讓例外或 panic 未經契約跨過外部 ABI。
- [ ] 每種資源都測試正常完成、提早返回、取消、配置失敗與部分初始化。

### 完成條件

**一般開發者能安全寫出可成長的資料結構、返回資料的函式、iterator、closure、錯誤處理及多模組函式庫，不必為了基本容器退回 legacy 或外部語言。**

不要求先實作 Rust 的每一個進階特性，但未支援範圍必須精確列出，不能妨礙 v1.0 承諾的正式專案。

---

## 七、工程主線 R3：正式原生編譯與合理的資源模型

**對應舊路線圖：M3。**

目前 native preview 對函式、slot、call depth、storage 與執行 fuel 設有明確上限，也拒絕遞迴；這些適合受限驗證，但不能直接成為一般服務與專業軟體的預設執行語意。

### 工作清單

- [ ] 保留既有 bounded profile 作為測試及受控執行情境，另建立一般 hosted execution policy。
- [ ] 一般 native 程式不因執行超過固定一百萬步就失敗；長時間服務可持續執行。
- [ ] 不直接刪掉防護：compiler resource limits、程式 runtime policy、OS 資源限制分別處理。
- [ ] 將固定小容量資料結構改為經檢查的可成長儲存，補足 OOM、overflow 與退化複雜度測試。
- [ ] 保留 OIR verifier 與 ownership witness 邊界，擴展功能時同步擴展驗證。
- [ ] 從固定 `-O0` 擴展到可用的 debug/release profile，加入最佳化正確性與效能回歸測試。
- [ ] 支援 executable、static library、shared library、debug information 與標準 linker 整合。
- [ ] 完成 Linux x86_64、Linux ARM64、Windows x86_64、macOS x86_64、macOS ARM64 五個原生平台。
- [ ] compiler、LLVM 與 SDK 支援版本明確鎖定；使用者不必知道每個內部 tool 的命令列。
- [ ] LTO、PGO、SIMD 等按實際效益加入；自有 machine-code backend 不阻擋第一個正式版本。

### 完成條件

在固定測試機器與資源預算內，成功處理至少一個**十萬行有效、非重複填充原始碼的多模組專案**，完成 clean／incremental build、除錯及 native 執行。

這是規模驗收下限，不是保證所有同等行數程式都有相同成本，也不是新的語言硬上限。

---

## 八、工程主線 R4：標準庫、Runtime 與真實服務

**對應舊路線圖：M4。**

### 工作清單

- [ ] 分離 `core`、`alloc`、`std`，讓系統程式與未來嵌入式不必連入整套服務 runtime。
- [ ] 完成 byte buffer、UTF-8 字串、集合、iterator、`Option/Result`、時間及隨機數介面。
- [ ] 提供一般二進位檔案與串流 I/O，補完整讀寫、部分讀寫、EOF、flush、逾時與取消契約。
- [ ] 文字解碼建立在 bytes 之上，正確處理跨 read 邊界的多位元組字元。
- [ ] 實作真正的 threads、同步原語、bounded channel、atomics 與非同步 executor／reactor。
- [ ] 完成 task cancellation、deadline、backpressure、公平性與 blocking work 分流。
- [ ] 提供標準程序參數、環境變數、signal、child process、退出碼與正常關閉。
- [ ] 以正式套件提供 HTTPS、HTTP client/server、JSON、WebSocket 與必要資料庫 driver。
- [ ] 允許採用經審查的成熟原生函式庫；不要求全部重写，也不以外部 Python 程序冒充 Oxid 原生 runtime。
- [ ] 補 structured logging、metrics、tracing、錯誤鏈及可診斷的資源洩漏資訊。

### 驗收專案

一個含資料庫、背景工作、連線池與 HTTPS 的服務，通過：

**至少 48 小時負載測試、取消與逾時測試、資料庫中斷、連線耗盡、部分寫入、程序重啟及 graceful shutdown。**

---

## 九、工程主線 R5：套件、生態與供應鏈

**對應舊路線圖：M5。**

### 工作清單

- [ ] 將套件識別改為名稱、版本、來源共同構成，支援同名多版本與 dependency rename。
- [ ] 完成 workspace、語意版本解析、features、平台條件與 host／target dependency 分離。
- [ ] 建立官方 registry、private registry、mirror、publish、yank、搜尋與下載流程。
- [ ] 鎖檔記錄完整解析結果；支援 locked、offline、frozen、vendor 與損壞快取復原。
- [ ] 用密碼學摘要驗證內容，另處理發布者身分、簽章、撤銷及金鑰輪替。
- [ ] build script／macro 的檔案、網路、程序權限清楚宣告；不把普通 subprocess 當成已隔離沙箱。
- [ ] 建立 SBOM、授權資訊、依賴安全通報與安全更新流程。
- [ ] 官方套件與 compiler 分開版本管理，但維持經測試的相容矩陣。
- [ ] 為常見需求提供可維護的標準選擇：CLI、設定、JSON、HTTP、資料庫、logging、測試、數值計算。
- [ ] 套件文件必須說明 thread safety、阻塞行為、配置成本與外部依賴。

### 完成條件

不熟悉 Oxid 內部的使用者能發布一個套件，由另一台乾淨機器下載、鎖定、離線重建及升級；惡意或損壞的來源變更會被拒絕。

---

## 十、工程主線 R6：讓日常使用真正優於「自己拼工具」

**對應舊路線圖：M7；從 R1 開始並行，不等 compiler 全部完成。**

### 工作清單

- [ ] LSP 提供 completion、hover、definition、references、rename、跨檔案診斷及自動修正。
- [ ] formatter、linter、文件工具共用 compiler 語法與語意，不維護另一套容易漂移的 parser。
- [ ] test runner 支援一般測試、compile-fail、doctest、測試篩選與可讀失敗報告。
- [ ] debugger 支援 breakpoint、stack、local variables、容器顯示及基本 async 診斷。
- [ ] 提供 CPU、配置量、記憶體及 compiler 耗時分析。
- [ ] `run`、REPL 與正式 build 使用同一套穩定語意；互動模式的特殊狀態清楚說明。
- [ ] 提供一條官方推薦的安裝及編輯器設定流程，避免使用者自行挑選十幾個不相容工具。
- [ ] 錯誤訊息說明原因、來源與修正方向；不能只有 E-code 或內部 OIR 名詞。
- [ ] 文件包含入門、語言手冊、標準庫、Rust 移轉、Python 互通、服務部署及問題排查。
- [ ] 文件範例透過真正發布的工具鏈執行，不只對倉庫內特殊 runner 有效。

### 使用者驗收

由至少 **5 位非核心開發者**依公開文件完成安裝、建立專案、加套件、定位錯誤、除錯、部署及升級。

每個必要任務至少 **4/5 人不依靠作者私下指導即可完成**；阻斷流程的問題必須回到正式 issue 修正。

這是小樣本發布檢核，不是市場優越性的統計證明。

---

## 十一、工程主線 R7：Rust／Python 互通與移轉

**對應舊路線圖：M6，並補強 Python 生態接入。**

### 11.1 Rust 路線

- [ ] 把「改用 Oxid 開發新專案」「移植 Rust 原始碼」「連結 Rust 函式庫」分成三種不同能力。
- [ ] 先完成 C-compatible ABI、header／binding 生成、callback、buffer ownership 與 allocator 邊界。
- [ ] 建立固定版本的 Rust 對照案例，不以無法維護的「全部 crate 相容」作 v1.0 承諾。
- [ ] 提供 Cargo metadata／manifest 匯入與逐步遷移報告。
- [ ] 轉換器不能靜默丟棄 lifetime、trait bounds、unsafe 契約或同步行為。
- [ ] 對必要 Rust crate 提供移植、官方替代套件或明確的 FFI 過渡路徑。
- [ ] 巨集相容另列矩陣，不以少數簡單文字替換宣稱 procedural macro 相容。

不應假設一般 Rust 型別具有可直接共用的固定 layout；Rust Reference 只承諾特定 representation 的保證。跨語言邊界應使用明確定義的資料表示。[Rust 文檔](https://doc.rust-lang.org/stable/reference/type-layout.html)

### 11.2 Python 路線

- [ ] 提供「Python 呼叫 Oxid 原生函式」的正式擴充建置流程，而不只有啟動 `oxid` 子程序。
- [ ] 支援 Python packaging 所需的 metadata、建置及 wheel 發布整合；不要另造一套只能手動安裝的 Python 發布方式。Python 封裝已有正式 `pyproject.toml` 規格可作整合邊界。[packaging.python.org](https://packaging.python.org/en/latest/specifications/pyproject-toml/)
- [ ] 明確處理 Python 物件生命週期、例外、執行緒及支援的 interpreter／ABI 組合。
- [ ] 加入 array／tensor 資料互通；零複製只能在符合 layout、ownership 與同步條件時使用。
- [ ] Python embedding、subprocess bridge、原生 extension 分別標示能力與成本。
- [ ] 使用 Python 套件時承認其 runtime 依賴；純 Oxid 程式不因此強制安裝 Python。

### 完成條件

同一份 Oxid 原生函式庫能被 C、Rust 與 Python 專案使用，且各自通過錯誤、回呼、跨執行緒及資源釋放測試。

---

## 十二、工程主線 R8：真正自宿主，不再停在元件示範

**對應舊路線圖：M8。可以逐項進行，但不能讓自宿主工作阻止一般用途語言基礎先完成。**

目前 explicit lexical provider 已是真實接點，但 Rust 仍負責 parser、semantic、ownership、OIR 與 backend authority；現有 `bootstrap/self-host` 的 round-trip 仍不能當成 compiler rebuild。

### 工作清單

- [ ] 先讓正式語言具備編譯器實作需要的字串、動態容器、檔案、錯誤處理及資料結構。
- [ ] 將已驗證的 Oxid lexer／parser 元件轉成可處理一般受支援輸入的正式 compiler module。
- [ ] 定義 provider 的實際輸入輸出、來源綁定、診斷及失敗語意。
- [ ] 替換一個 provider 時，完整測試它與後續階段的整合，而不只比對固定 transcript。
- [ ] Rust oracle 逐步轉為測試用 differential checker；自宿主正式路徑不得仍強制呼叫 Rust 語意分析才能成立。
- [ ] 對已通過切換條件的 provider 禁止 fallback；失敗應報錯，而不是偷偷換回舊路徑。
- [ ] Oxid compiler 必須編譯完整 compiler source，也能編譯未硬編碼在測試中的其他合法程式。
- [ ] 建立 C1、C2、C3 真正重建流程，固定 source、target、依賴與旗標。
- [ ] 在同一 target 的可重現建置設定下比較 C2/C3，不跨不同 CPU 或作業系統要求 native binary hash 相等。
- [ ] 在無 Rust／Cargo 環境中，由公開 Oxid 種子重建指定 compiler 與標準庫。
- [ ] 公開 trusted seed、LLVM／系統工具依賴與重建證據；不把「自宿主」誤寫成所有外部工具都由 Oxid 重寫。

### 完成條件

**使用者拿到可驗證的 Oxid 種子工具鏈與原始碼，就能依公開程序重建指定版本，過程沒有隱藏 Rust compiler 元件。**

---

## 十三、工程主線 R9：原生數值與 AI，分三層完成

這條主線延續原本的 AI 目標，但把 v1.0 與後續硬體擴充分開，避免整個語言版本被大型訓練需求無限延後。

### A：v1.0 必須完成的 CPU 基礎

- [ ] typed buffer、shape、stride、layout、dtype、broadcasting 與 checked indexing。
- [ ] CPU tensor、基本線性代數、reduction 與必要數值算子。
- [ ] reverse-mode autograd、梯度累加、no-grad、detach 及自訂 gradient。
- [ ] SGD、Adam／AdamW、scheduler、gradient clipping 及 deterministic seed。
- [ ] dataset、batching、shuffle、資料讀取與訓練／驗證流程。
- [ ] checkpoint 保存模型、optimizer、RNG 與資料進度。
- [ ] 完成至少一個 MLP 與一個較複雜模型的原生 CPU 訓練案例。
- [ ] 驗收 forward、gradient、更新步驟、收斂與恢復結果，不只看程式能跑。
- [ ] 訓練核心不能實際委派給外部 Python 框架，卻標示為 Oxid 原生訓練。

### B：v1.x 的 GPU 訓練完成條件

- [ ] 第一個 GPU backend 在固定、實際可測硬體上完成。
- [ ] device allocation、stream、event、非同步 copy 與裝置錯誤處理。
- [ ] buffer 在 GPU 工作結束前不得提前釋放。
- [ ] mixed precision、必要高精度累加及 loss scaling。
- [ ] tensor／graph IR、fusion、layout planning、memory planning 與 kernel profiling。
- [ ] GPU 與 CPU 參考結果在預先定義的數值容許範圍內一致。
- [ ] 從零訓練、微調、OOM 診斷與 checkpoint resume 均有真實案例。
- [ ] 新增 AMD／Apple 等 backend 時，獨立驗證，不繼承其他硬體的成功標籤。

### C：成熟 AI 平台的完成條件

- [ ] data parallel、collective、參數／optimizer sharding。
- [ ] 多 GPU、多節點、拓樸、通訊與計算重疊。
- [ ] worker failure、timeout、資料分片及分散式 checkpoint 恢復。
- [ ] tensor／pipeline parallel 與大型模型記憶體策略。
- [ ] 訓練資料版本、實驗設定、模型來源與可追溯紀錄。
- [ ] 比較達到同等品質所需時間與成本，不只比較單一步驟吞吐量。

**大型 GPU 訓練是完整的後續交付目標，不是用 Python bridge 提前勾選完成，也不是 v1.0 永遠不能發布的理由。**

---

## 十四、真正决定 v1.0 的六個正式專案

這六個專案應與 compiler fixtures 分開維護，使用一般使用者能取得的工具與介面。

| 專案 | 必須證明的能力 |
|---|---|
| **原生 CLI／資料處理工具** | Unicode、二進位檔案、串流處理、參數、錯誤、跨平台發布；能處理大於 RAM 的資料流程。 |
| **HTTPS／資料庫服務** | JSON、資料庫交易、連線池、並行 I/O、觀測、正常關閉與故障恢復。 |
| **背景工作程序** | 任務排程、執行緒、程序控制、signal、取消、重試與長時間資源穩定性。 |
| **跨語言原生函式庫** | C／Rust／Python 實際整合、ABI、資料所有權、callback 與發布。 |
| **原生 CPU 訓練應用** | tensor、autograd、optimizer、資料流程、收斂及 checkpoint resume。 |
| **Oxid compiler／工具鏈** | 真正自宿主、多模組工程、可重現建置及日常維護能力。 |

共同要求：

- [ ] 全部使用一般 `build/test/package` 路徑，不依靠 fixture 專用後門。
- [ ] 建置、部署、更新、回退與問題排查都有文件。
- [ ] 至少三個有非 compiler 核心作者參與維護。
- [ ] 至少完成一輪跨版本升級，不能只證明某個快照能跑。
- [ ] 對必要應用進行至少 30 天試用或實際使用，記錄問題及處理結果。

這比新增數百個只能回傳固定數字的範例，更接近「使用者願意把 Oxid 當主力語言」。

---

## 十五、效能、可靠性與發布門檻

### 15.1 效能：競爭力是必要條件，全面勝出不是版本定義

建議對預先固定的核心 CPU workload，在指定基準平台採用以下初始預算：

| 指標 | 建議 v1.0 門檻 |
|---|---|
| 執行時間 | 對等 Rust 實作的幾何平均比值不高於 1.25。 |
| 單項嚴重退步 | 任一必要 workload 不高於 Rust 的 2 倍。 |
| 長時間記憶體 | 同等功能、設定及負载下，必要應用的峰值 RSS 不高於對照的 1.5 倍；特殊情況不得事後隱藏。 |
| 正確性 | 效能比較前，輸出、錯誤處理與安全語意一致。 |
| 統計 | 短測試保存至少 30 個有效樣本、環境、原始數據與信賴區間。 |
| AI | 另測梯度、收斂、記憶體與 time-to-quality，不沿用純 CPU loop 的門檻。 |

這些比值是**建議的發布預算**，不是 Oxid 現有成績；也不代表其他平台自動達標。跨平台能力與跨平台效能宣告必須分開。

最佳化不得靠默默移除已承諾的 bounds／overflow／ownership 規則取得優勢。

### 15.2 可靠性與安全

- [ ] 穩定範圍沒有已知阻斷性的錯誤編譯、soundness、資料損毀或重大安全問題。
- [ ] 對支援功能建立正向、compile-fail、邊界、組合、fuzz 與永久回歸測試。
- [ ] compiler crash 有可保留證據及縮減輸入的流程。
- [ ] 對 unsafe、FFI、allocator、並行 runtime 與 artifact reader 進行專門審查。
- [ ] 支援平台執行適用的 sanitizer／動態檢查。
- [ ] 「沒有已知缺陷」不寫成「證明不存在任何缺陷」。

### 15.3 穩定性與维护

- [ ] v1 公開語意與 API 在相容版本中維持相容。
- [ ] 破壞性變更經 edition 或明確 major policy 處理，不用更新預設值偷偷改變舊程式。
- [ ] 建議 v1.0 穩定系列至少提供 24 個月安全維護，發布前確認實際負責人與資源。
- [ ] 發布 artifact 具有來源、摘要、簽章、支援平台與外部依賴清單。
- [ ] 能安裝指定版本、更新、回退及離線部署。
- [ ] 文件按版本提供，不把 `main` 的 experimental 能力混進穩定版指南。
- [ ] 連續兩個 release candidate 通過全部必要關卡；核心變更後重新執行受影響的驗收。

---

## 十六、v1.0 之後：一路到成熟的通用主力語言

後續使用**能力認證**管理，不先假定每項新功能都必須升一個 major version。

| 成熟能力 | 必要工程 | 完成證據 |
|---|---|---|
| **更廣 Rust 移轉** | 泛型／trait 進階能力、巨集、Cargo 工作流程與更多 crate 移植。 | 固定版本、固定範圍的真實 Rust 專案可持續移轉及維護。 |
| **Python 級互動工作流程** | REPL、notebook、資料探索、套件體驗及原生擴充工具。 | 從探索到原生部署不必重寫整個專案；成本與動態邊界明確。 |
| **成熟 AI 訓練** | GPU、tensor compiler、分散式、混合精度及大型模型恢復。 | 不同模型與硬體的完整訓練、品質及效能報告。 |
| **科學計算／HPC** | 複數、稀疏矩陣、FFT、SIMD、分散式數值計算。 | 真實模擬或科學應用的精度與擴展測試。 |
| **WASM／Edge** | WASM、明確版本的 WASI、host capability 與封裝。 | 真實服務及邊緣應用部署，不只生成模組。 |
| **嵌入式／RTOS** | `no_std`、HAL、中斷、MMIO、DMA、allocator 與韌體流程。 | 真實板子上的長時間運行、更新及故障恢復。 |
| **桌面／行動** | GUI、事件、輸入、無障礙、Android／iOS 整合。 | 可安裝、更新與維護的真機應用。 |
| **圖形／遊戲／音訊** | rendering、輸入、音訊、資產流程與 frame-time 分析。 | 持續互動負載與正式發布案例。 |
| **系統／儲存** | 裸機、kernel／driver 邊界、持久化與 crash consistency。 | 開機、載入或故障注入後維持指定契約。 |
| **長期生態健康** | 套件維護、相容性追蹤、安全更新與治理。 | 非核心作者能獨立開發、發布與維護重要套件。 |

**最終成熟目標不是一次性的「功能全部完成」，而是每個宣告支援的領域，都有穩定工具、套件、真實使用者、可重現證據與持續維護能力。**

---

## 十七、直接對應目前倉庫的執行順序

以下既有路徑已由目前倉庫確認；「新增」項目是建議路徑，不代表已存在。現有 frontend 已分成 driver、options、project、typeck、HIR、OIR 與 native 模組，不需要為了新路線圖全部推倒重排。 

| 優先順序 | 主要位置 | 可獨立驗收的交付 |
|---|---|---|
| **1** | `docs/ROADMAP.md`、`docs/feature-status.json`；新增 `docs/RELEASE_CRITERIA_1_0.md` | 固定 G1–G10、owner、證據與發布範圍。 |
| **2** | `src/frontend/driver.rs`、`options.rs`、`project.rs`、`src/main.rs` | typed manifest 與一般 `build/run/test` 整合；保留 legacy 相容。 |
| **3** | `src/frontend/oir/`、相關 resource／execution 模組 | 分离 bounded 與 hosted policy，建立一般專案規模測試。 |
| **4** | `ast.rs`、`hir.rs`、`typeck.rs`、`oir/owned/`、`oir/owned_types.rs` | 一般字串、堆積容器、析構及跨 scope 借用；每次擴展都包含端到端使用。 |
| **5** | `src/frontend/native.rs`、`src/frontend/oir/native.rs`、`native/` | 正式 release profile、除錯資訊及第二個原生 target，再擴至五平台。 |
| **6** | `src/runtime/`、`stdlib/`、`packages/` | typed-native I/O、真正 runtime、套件及第一個正式 CLI／服務。 |
| **7** | `src/frontend/lexical_provider.rs`、`hir_producer.rs`、`compiler/` | 將成熟元件接成正式 compiler provider，逐步移除 Rust 必經語意路徑。 |
| **8** | `.github/workflows/ci.yml`、`scripts/`；新增 release gate runner | 發布關卡依必要工作結果決定，不只看整個 workflow 綠燈。 |

### 執行上的三項調整

**第一，測試框架與驗證報告應服務於使用者能力。**  
保留現有嚴謹度，但新增功能必须伴隨可觀察的使用者成果，不再無限增加只驗證驗證器的工作。

**第二，一般用途基礎優先於更多窄範圍 compiler demo。**  
typed project、字串、動態容器、一般檔案 I/O、release native build，應先於持續新增容量受限的 lexer／parser 示範。

**第三，以完整流程推進，而不是等每一層「理論上全部完成」。**  
先交付能正常建置、測試、部署的小型正式專案，再擴展語言與生態；同時維持安全邊界與回歸測試。

---

## 十八、最終完成條件

### Oxid v1.0 完成

**G1–G10 全部通過，六個正式專案可由一般使用者完成開發、部署與升級，指定工具鏈具真正自宿主，且穩定版本開始承擔相容性與維護承諾。**

### Oxid 成為主要 Rust 替代語言

**開發者在已認證領域選擇 Oxid 時，不需要以缺乏容器、套件、除錯工具、部署流程或可靠性為代價；移轉成本、效能、可維護性與日常開發體驗，都有實際案例支持。**

### Oxid 達到成熟通用語言目標

**這種能力不再依赖作者本人陪同操作，而能由外部團隊、套件維護者及不同領域使用者，跨版本、跨平台持續重現。**

**這三個完成層級應分開追蹤。v1.0 有明確終點，Rust 主力替代有真實專案標準，成熟通用語言則以持續擴大的領域認證與維護能力完成。**