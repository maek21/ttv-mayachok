# Маячок

Голосовой ввод для Windows. Зажми **Ctrl + Win**, говори, отпусти — текст появится там, где стоит курсор, в любом приложении.

Макеты: [Маячок — макеты](https://claude.ai/artifact/XtKQfCrF5eriyepQ2itG9C) (Dynamic Island-стилистика).

## Что умеет

- **Остров** — чёрная «капсула» поверх всех окон в стиле Dynamic Island: волна от реальной громкости, таймер, живой текст во время записи, плавный морфинг между состояниями (запись → обработка → «Вставлено · 11 слов»). Не забирает фокус и не кликается.
- **Горячие клавиши** через низкоуровневый хук: удержание (Ctrl + Win), нажать/нажать, режим «без рук» (Ctrl + Win + Space), отмена (Esc), повторная вставка последней фразы (Alt + Shift + V), открыть окно (Ctrl + Alt + M). Любое сочетание можно переназначить — даже из одних модификаторов. Отпускание Win не открывает «Пуск».
- **Распознавание локально** — whisper.cpp (large-v3-turbo q5 / full, small, base), модель качается из приложения, работает офлайн. Или **облако** — любой OpenAI-совместимый `/audio/transcriptions` (OpenAI, Groq…).
- **Чистка текста** — авто-пунктуация, удаление паразитов («э-э», «ну,», «типа,»), голосовые команды («новая строка», «новый абзац»), замены и сниппеты из словаря, свои слова как подсказки модели. Фильтр «галлюцинаций» Whisper на тишине.
- **Вставка** через буфер обмена + Ctrl+V с восстановлением прежнего содержимого буфера.
- **История** с поиском, фильтрами по приложениям, избранным, прослушиванием аудио, копированием и повторной вставкой. Экспорт в JSON/Markdown, срок хранения, исключённые приложения (менеджеры паролей и т.п.).
- **Главная** — слова за день, время диктовки, сэкономленное время, серия дней, график за неделю.
- **Трей** — быстрый старт, копировать последнее, выбор микрофона и языка, пауза на час.
- Автозапуск с Windows, сворачивание в трей, тёмная/светлая/системная тема, выбор цвета маячка, звуки старта/стопа.

## Шрифты

- Заголовки — **Geologica** (свободная альтернатива Claude/Anthropic Sans: сам Anthropic Sans проприетарный и в приложение не кладётся).
- Всё остальное — **Roboto Flex** (вариативный, с кириллицей; у Google Sans Flex на Google Fonts кириллицы нет). Текст постоянно «дышит»: ось ширины `wdth` 100→105 и грейд `GRAD` −25→70 по кругу за 7 секунд. Амплитуда настраивается, анимация замирает, когда окно не в фокусе, и уважает `prefers-reduced-motion`.

Шрифты вшиты в сборку (Fontsource), интернет для них не нужен.

## Стек

- [Tauri 2](https://tauri.app) + Rust — ядро, окна, трей, автозапуск.
- React 19 + TypeScript + Vite — интерфейс.
- `whisper-rs` (whisper.cpp), `cpal` (микрофон), `rusqlite` (история), WinAPI через `windows` (хук клавиатуры, SendInput, окно без активации).

```
src/                    интерфейс
  screens/              Onboarding, Home, History, Dictionary, Settings
  island/IslandApp.tsx  окно-остров
  components/           UI-кит, каркас окна, IslandView
src-tauri/src/
  engine.rs             запись → распознавание → чистка → вставка → история
  hotkey.rs             WH_KEYBOARD_LL, сочетания, захват нового сочетания
  platform.rs           вставка, активное приложение, окно без фокуса, звуки
  transcribe.rs         whisper.cpp и облако
  postprocess.rs        паразиты, команды, замены (+ тесты)
  audio.rs  models.rs  history.rs  settings.rs  tray.rs  island.rs  commands.rs
```

## Сборка

Нужны Node 22, Rust stable, на Windows — Visual Studio Build Tools (C++), CMake и LLVM (для bindgen: `LIBCLANG_PATH`).

```bash
npm ci
npm run tauri dev                      # разработка
npm run tauri build -- --bundles nsis  # установщик .exe
```

### Видеокарта или процессор

- **Vulkan (основная сборка)** — `npm run tauri build -- --features vulkan`, нужен [Vulkan SDK](https://vulkan.lunarg.com/) (`VULKAN_SDK`). Работает на NVIDIA, AMD и Intel; exe требует `vulkan-1.dll`, который ставится вместе с драйвером видеокарты. Если видеокарта не нашлась — считает на процессоре.
- **CPU** — обычная сборка без фич. Также есть `--features cuda` (нужен CUDA Toolkit).

whisper.cpp всегда собирается под **AVX2 + FMA + F16C** (`src-tauri/cmake/ggml-cpu.cmake`, подключается через `.cargo/config.toml`). Без этого ggml либо подстраивается под процессор машины сборки (на CI — AVX-512, и exe падает на домашних CPU), либо при кросс-сборке откатывается до SSE4.2 и работает в разы медленнее. После правки этого файла пересобери whisper: `cargo clean -p whisper-rs-sys`.

Ещё два ускорения на процессоре: `audio_ctx` по длине фразы (whisper иначе всегда считает 30-секундное окно) и flash attention. На 4-ядерном Xeon 2,1 ГГц фраза в 5 секунд на large-v3-turbo q5: 26 с → 5,7 с.

Кросс-сборка из Linux: `cargo install cargo-xwin`, clang ≥ 19 как `clang-cl`, `nsis`, затем
`npx tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis`.

CI (`.github/workflows/build.yml`) на каждый пуш собирает два установщика под Windows — `mayachok-windows-vulkan` и `mayachok-windows-cpu` — и кладёт их в артефакты; на тег `v*` — создаёт релиз.

## Данные

Всё хранится локально в `%APPDATA%\app.mayachok.desktop`: `settings.json`, `history.db`, `audio/`, `models/`. Облачный API-ключ лежит в `settings.json` открытым текстом.
