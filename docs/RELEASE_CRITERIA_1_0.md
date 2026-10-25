# Oxid v1.0 技術發布驗收矩陣

**目前狀態：v1.0 未達；G1–G10 均尚未通過，G5 持續開發中。** 本頁列出目前有效的技術發布條件與開發分支的能力範圍。[ROADMAP_V2.md](ROADMAP_V2.md) 保留為原始工程方向參考；具體語意由對應規格與 RFC 說明，技術缺口同步記錄於 [release-gates-v1.json](release-gates-v1.json)。

## 目前開發狀態

- [原生專案流程](TYPED_PROJECTS.md) 支援建立專案、檢查程式、執行、建置、測試與格式化。設定集中於 `oxid.toml`；建置成功才替換執行檔，失敗時保留上次的產物。目前功能仍屬實驗性質。
- Linux 的 typed 本地路徑依賴已接入單一 compiler session，保留套件自己的 `crate::`、可見性、所有權與型別身分。[Byte count](../apps/byte-count/README.md) 是 fixture 之外的三套件應用；其獨立驗證涵蓋直譯／原生輸出、依賴修改、建置失敗保留產物與移除來源後部署。typed 鎖檔另記錄本機依賴的實際載入來源，支援 locked／offline／frozen 檢查。typed workspace 已加入成員選擇、library 檢查與涵蓋完整依賴圖的共用鎖檔，並設有資源上限；來源封裝已加入完整本地依賴配置、保留輸入驗證與原子解壓；可選的相對路徑 native 建置支援固定工具鏈與保留來源配置下的搬移重建；registry、vendor、逐模組增量建置與更廣泛的產物驗收仍待完成。
- CI 已配置五個編譯器 host：Linux x86_64、Linux ARM64、Windows x86_64、macOS x86_64、macOS ARM64。各 host 的結果說明 Rust 實作的編譯器在該環境的行為；Oxid native 已實作 **Linux x86_64／ARM64、LLVM 19.1.7、`-O0`／`-O2` 與現有執行限制**；ARM64 實機驗證仍待完成。
- `oxid build --profile dev` 使用 `-O0`，`--profile release` 使用 `-O2`，同時套用於程式與 runtime；`bounded` 是 `dev` 的別名。最佳化與執行政策分開設定；`[execution] policy = "hosted"` 可移除累積執行額度，型別、借用、算術、邊界、記憶體與編譯器限制仍保留。可用 `build --debug-info` 加入函式名稱與來源行號；區域變數檢視仍待完成。Rust 編譯器自身的 debug／release 建置另行驗證。
- `build --incremental` 可選用 LLVM object 快取；每次仍檢查來源、驗證鎖檔、編譯 runtime 並重新連結 SDK。這項能力尚不包含逐模組 frontend 增量編譯，亦不代表原生產物已通過可重現性或效能驗收；見 [RFC 0036](../rfcs/0036-native-object-reuse.md)。
- `build --reproducible` 使用相對於專案／工作區的執行時診斷路徑；驗證獨立比對 clean、快取與搬移建置的原始執行檔。範圍限相同 compiler、LLVM／SDK、設定與環境，見 [RFC 0039](../rfcs/0039-reproducible-native-builds.md)。
- `doctor --native` 透過實際的工具版本、SDK 編譯、連結與執行探測診斷環境，亦檢查目前 typed 專案與既有鎖檔；見 [native 診斷契約](NATIVE_DOCTOR.md)。

實作與驗證範圍見 [RFC 0032](../rfcs/0032-typed-project-workflow.md)、[RFC 0033](../rfcs/0033-typed-path-dependencies.md)、[RFC 0034](../rfcs/0034-typed-local-locking.md)、[RFC 0035](../rfcs/0035-typed-workspaces.md)、[RFC 0038](../rfcs/0038-typed-source-packages.md)、[typed preview 規格](../spec/typed-preview.md) 與 [project workflow validation](architecture/project-workflow-validation.md)。CI 設定、工作分支功能與已發布版本分別記錄，不以設定存在或排程成功代替實際驗證結果。

## v1.0 範圍

十個技術關卡必須全部通過。發布範圍包括穩定 typed edition、語意一致的直譯與 native 執行、上述五個 native 平台、完整日常專案流程、六個正式應用、原生 CPU 訓練，以及真正的編譯器自宿主。

GPU／分散式訓練、完整 Rust crate／procedural macro 相容、完整 Python 動態語意，以及 WASM、嵌入式與其他領域認證，保留為後續目標。

| 關卡 | 必要技術驗收 | 現況與剩餘工作 |
|---|---|---|
| **G1 語言穩定** | 版本化規格、支援矩陣、相容性／棄用／遷移政策與 conformance tests 相互對應；相容版本維持公開語意與 API，破壞性變更經 edition 或 major policy 處理。 | 未達。`typed-preview` 仍為 experimental；未指定 edition 的舊專案保留 legacy 行為。尚缺完整穩定規格與驗收套件。 |
| **G2 安全核心** | 一般堆積配置、字串、可成長容器、借用、析構與並行安全契約可用；正向、compile-fail、邊界及組合案例覆蓋資源生命週期。 | 未達。目前以 bounded records、固定陣列、call-only borrowing、slice 與部分 enum／match 為主；一般容器、析構、儲存／回傳參考及並行安全仍不完整。 |
| **G3 正式原生編譯** | 五平台 native 建置、執行與除錯；可用的 debug/release profile、最佳化正確性、executable／static／shared library、可重現 clean／incremental build，以及一般 hosted execution policy。規模驗收建議採用十萬行有效、非重複填充的多模組專案。 | 未達。現有 native 專案已實作 Linux x86_64／ARM64 的 LLVM `-O0`／`-O2` 與選用的來源行號資訊；ARM64 實機驗證、固定資源限制、其餘 targets、完整除錯與一般增量建置驗收仍待完成。五個編譯器 host 的 CI 不構成五平台 native 驗收。 |
| **G4 完整 Runtime** | typed-native 檔案／二進位串流、程序、執行緒、同步、非同步 I/O、取消與資源釋放具有正式契約；HTTPS／資料庫服務通過負載測試（建議 48 小時）及逾時、取消、斷線、連線耗盡、部分寫入、重啟與正常關閉測試。 | 未達。legacy 的檔案、程序、網路與 lazy task 功能尚未形成完整 typed-native runtime；一般 I/O、真正並行及取消契約仍有缺口。 |
| **G5 套件與建置** | 同一套設定貫通 `new → build → run → test → package`；typed dependency graph、workspace、多版本依賴、registry、鎖檔、offline/frozen/vendor、完整性驗證與 cache invalidation 可用。fixture 之外的多套件應用須完成修改模組、增量建置、測試與乾淨部署。 | 進行中，未達。experimental 單套件流程已接入 manifest；Linux typed path dependencies 已接入同一 compiler session，驗證範圍依實際結果記錄。bounded workspace 的成員流程、共享鎖定與逐 binary 原生 object reuse 已有 experimental 實作；本地來源封裝已有 experimental 實作；registry、多版本解析及完整增量流程尚未完成。legacy 的套件與 SHA-256 功能只證明其既有範圍。 |
| **G6 開發工具** | LSP、formatter、test runner、debugger、版本化文件與 profiler 使用一致的專案設定；安裝、編輯、格式化、測試、除錯及效能分析可透過公開工具流程重現。 | 未達。已有 typed 單檔 formatter、manifest 測試流程及編輯器設定；已加入 GDB 函式／行號斷點與呼叫堆疊；區域變數檢視、跨檔 LSP、文件產生與 profiling 尚未形成完整工作流程。 |
| **G7 互通與遷移** | 同一份 Oxid native library 與 C、Rust、Python 實際整合；驗證 ABI、錯誤、所有權、callback、跨執行緒與資源釋放；Python native extension 可用標準方式打包，遷移工具明列不支援範圍。 | 未達。既有 subprocess adapters 尚未提供上述原生函式庫與擴充模組工作流程。 |
| **G8 真正自宿主** | 公開 Oxid 種子在無 Rust／Cargo、無隱藏 Rust frontend fallback 的流程中重建指定 compiler 與標準庫；C1/C2/C3 重建達到同 target 的 C2/C3 固定點，並能編譯其他合法程式。 | 未達。Rust 仍掌握 parser、semantic、ownership、OIR 與 backend；bounded 元件和 AST encode/decode round-trip 尚未構成 compiler 重建。 |
| **G9 正式應用與效能** | 下列六個應用走一般 `build/test/package` 與部署／升級流程；固定 workload 驗證正確性、壓力、恢復及效能。CPU tensor、autograd、optimizer、資料流程與 checkpoint resume 通過數值、梯度、收斂、記憶體及恢復測試。 | 未達。完整六應用、原生 CPU 訓練基礎與對照效能 corpus 尚未完成；現有 benchmark harness 主要量測 Oxid 自身。 |
| **G10 發布與升級** | artifacts 具有來源 provenance、摘要、簽章、平台及外部依賴清單；指定版本安裝、更新、回退與離線部署可重現，文件按版本提供。連續兩個 RC 通過全部技術關卡，核心變更後重跑受影響驗收。 | 未達。現有 release workflow 打包四個編譯器 host 並附 checksum；五平台 native 交付、簽章／provenance、完整更新與回退流程及兩個合格 v1 RC 尚未完成。 |

## 六個正式應用與共通驗收

應用使用公開工具與介面，與 compiler fixtures 分開維護，並保存建置、部署、排錯、至少一輪跨版本升級及回退結果。

| 應用 | 必須驗證的能力 |
|---|---|
| 原生 CLI／資料處理工具 | Unicode、二進位檔案、串流、參數、錯誤處理、跨平台交付及大於 RAM 的資料流程。 |
| HTTPS／資料庫服務 | JSON、交易、連線池、並行 I/O、觀測、正常關閉與故障恢復。 |
| 背景工作程序 | 排程、執行緒、程序控制、signal、取消、重試與長時間資源穩定性。 |
| 跨語言原生函式庫 | C／Rust／Python ABI、資料所有權、callback 與打包整合。 |
| 原生 CPU 訓練應用 | MLP 與較複雜模型的 tensor、autograd、optimizer、資料流程、收斂與 checkpoint resume。 |
| Oxid compiler／工具鏈 | 自宿主、多模組專案、可重現重建與一般開發流程。 |

每個關卡同時適用以下技術要求：

- 穩定範圍沒有已知阻斷性的錯誤編譯、soundness、資料損毀或重大安全缺陷；保留正向、compile-fail、邊界、組合、fuzz 與永久回歸測試。compiler crash 應能保留證據及縮減輸入。
- unsafe、FFI、allocator、並行 runtime 與 artifact reader 經專項檢查，支援平台執行適用的 sanitizer／動態檢查。測試結果只說明已驗證範圍，不宣稱不存在任何缺陷。
- 效能比較先確認輸出、錯誤處理與安全語意等價；固定 corpus，保留環境、有效樣本、原始數據與信賴區間。CPU 訓練另量測梯度、收斂、記憶體及 time-to-quality；最佳化維持已承諾的 bounds／overflow／ownership 規則。
- 路線圖中的效能比值仍為建議預算（proposed）：相對 Rust 的時間幾何平均 ≤ 1.25、單項 ≤ 2、峰值 RSS ≤ 1.5，以及短測試至少 30 個有效樣本。驗收前須固定 workload、平台、量測方法與採用的門檻；沒有實測結果時不標示達標。

## 證據與發布判定

| 證據範圍 | 必須記錄 | 可支持的結論 |
|---|---|---|
| 歷史 | 當時 commit、工具版本、平台、命令與結果。 | 對應舊版本及已測範圍的結果。 |
| 目前開發版本 | 實際 commit 或明確標示的 worktree、輸入身分、建置、平台、命令、完整結果與產物摘要。 | 該版本在實際執行範圍內的結果；本地結果與 hosted CI 分別記錄。 |
| RC／正式發布 | tag 與 package version、實際發布 artifacts、來源與摘要，以及對應平台的驗收結果。 | 該發布版本具體交付能力與相容性。 |

目前 CI 將 `9ff8ef90a26f003f4616838dfc186fa3bfb6845c` 的歷史認證重播與當前 compiler／專案／native 回歸分開。歷史結果維持原有來源範圍；current 結果依其實際 commit 與平台判定，不能重新發出未執行的歷史私有認證。完整說明及重現命令見 [project workflow validation](architecture/project-workflow-validation.md)。

`scripts/check_release_gates.py` 檢查技術缺口清冊及 tag／`Cargo.toml` 版本一致性；完整 G1–G10 技術驗收尚未實作並取得通過結果前，stable v1 發布保持阻擋。修改清冊狀態、列出證據檔名或單獨取得 CI 綠燈，均不代替上述驗收。release workflow 仍須通過驗證後才打包並發布。

原始路線圖保留為 35,028 bytes，SHA-256 `09c787fd6ffe72db1d5aa1df9c49214b3e77e760d2689fe8e5d04405fa429b09`；本頁更新不改動其內容。
