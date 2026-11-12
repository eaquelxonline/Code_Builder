# Apk_Builder — Koordinasyon

Bu dosya projenin ortak hafızasıdır. Kalıcı kararlar ve güncel durum burada tutulur.
Bir iş bitince ya da yeni bir işe başlanınca ilgili bölüm güncellenir.

Proje bir süre iki ayrı oturumla (arka uçlar için A, dil ön yüzleri ve Android için B)
yürütüldü. İki dal `Apk_Builder_Beta` üzerinde birleştirildi; artık tek sistem, tek dal var.
Aşağıdaki "Claude-A" ve "Claude-B" adları o dönemin kayıtlarıdır.

## 1. Proje hedefi

Apk_Builder, Android cihazın üzerinde çalışan, dışarıdan hiçbir araç
(javac, kotlinc, d8, aapt2, apksigner, clang, ld) çağırmadan kaynak koddan
imzalı APK/AAB üreten bir derleme ortamıdır. Uygulamanın kendisi
Kotlin/Gradle ile yazılır; tüm derleme, paketleme, imzalama ve dil
derleyicileri Rust'ta, tek bir crate (`apk_builder_core`) içinde yazılır ve
uygulamaya JNI köprüsüyle (C++ `Builder.cpp`) bağlanır.

Uzun vadeli hedef: Java, Kotlin, C/C++ ve Rust kaynaklarını cihazda derlemek.
JVM tarafı için Dex üretimi, yerel kod tarafı için kendi ara gösterimimiz
(Omni IR), kendi makine kodu üreticimiz (x86, x86_64, ARMv7, ARMv8),
kendi assembler/disassembler'ımız ve kendi bağlayıcımız (ELF/COFF) kullanılır.
Bu bileşenlerin doğruluğu, LLVM/LLD/GCC'nin kendi test paketleriyle ölçülür.

## 2. Mimari ve klasör haritası

| Yol | Ne işe yarar |
| --- | --- |
| `Builder.rs` | Crate kökü (`apk_builder_core`). Derleme grafiği, zamanlayıcı, önbellek, VFS, Gradle/Maven okuma, kaynak (res/arsc/AXML) derleme, Dex yazma, R8 benzeri küçültme (`shrink`), desugar, APK/AAB paketleme, imzalama (v1–v4, RSA, ML-DSA, X.509), inflate/deflate, görüntü işleme, JNI `ffi` modülü ve `compilers` modül kaydı. |
| `Diagnostic_Speech.rs` | Tüm tanı cümlelerinin ve etiketlerinin 14 dile çevirisi (`*_LINES`, `*_WORDS`, İngilizce anahtara göre sıralı). |
| `Terminal.rs`, `Terminal_Speech.rs` | Uygulama içi terminal ve çevirileri. |
| `Vibe_Coding.rs` | Yapay zekâ destekli kodlama yardımcısı (katmanlar: keyless/free/byok). |
| `Android.rs` | Android platform API'sinin derleyiciler için gömülü kaynak tanımları (`THE_PLATFORM`). |
| `Compilers/Java/` | Java derleyicisi (`Java.rs`), tanıları ve otomatik düzelticisi. |
| `Compilers/Kotlin/` | Kotlin derleyicisi (`Kotlin.rs`), tanıları ve otomatik düzelticisi. |
| `Compilers/Cpp.rs`, `Compilers/Rust.rs` | C/C++ ve Rust ön yüzleri (şu an iskelet). |
| `Compilers/Compiler_Infrastructure/Compiler_Infrastructure.rs` | Omni IR: metin/ikili biçim, SSA, doğrulayıcı, yorumlayıcı. |
| `.../Compiler_Infrastructure_Diagnostic.rs` | `EIxxx` tanı kataloğu (sıralı). |
| `.../TableGen.rs` | LLVM TableGen dilinin kendi uygulaması; `.td` → ikili komut tabloları (`Tables/*.bin`, biçim sürümü 2, `Tables.manifest`). |
| `.../Tables/` | `X86.bin`, `X86_64.bin`, `Armv7.bin`, `Armv8.bin` (TableGen çıktısı, `include_bytes!` ile gömülü). |
| `.../Elf.rs`, `.../Pe.rs` | ELF ve COFF/PE okuma-yazma. |
| `.../Linker.rs` | `ld.lld` ve `lld-link` uyumlu bağlayıcı (ELF: x86/ARM/AArch64, COFF/PE, PDB). |
| `.../X86_64.rs`, `.../X86.rs` | x86 hedef tanımı, kodlayıcı (LLVM X86MCCodeEmitter ile birebir), AT&T/Intel yazıcı, AsmMatcher eşleyici, assembler (lexer, ifadeler, direktifler, makrolar, metin çıktısı). `X86.rs` 16/32 bit kipini ve `X86.bin`'i bağlar. |
| `Builder/` | Android uygulaması: `Source/Main/Kotlin/.../Builder.kt` (arayüz), `Source/Main/Native/` (JNI köprüsü, CMake), `build.gradle.kts`. |
| `Gradle/`, `build.gradle.kts`, `settings.gradle.kts`, `gradlew` | Gradle sarmalayıcı ve yapılandırma. |
| `New/Beta`, `New/Test` | Başvuru paketleri (LLVM, Clang, LLD, GCC, binutils, Rust, Kotlin, V8, ICU, TypeScript kaynakları ve test dosyaları). Testler bunları git nesnesi olarak `catalogue()`/`Tree::of` ile okur. |
| `Test/Builder_Test.rs` | Bütün Rust testleri (`mod tests`, her alanın kendi `mod`'u). Lit `RUN:` çalıştırıcısı ve FileCheck eşdeğeri burada. |
| `Test/Builder_Test.py` | Tüm kapı denetimleri (layout, comments, format, lint, strings, speech, independence, suites, linker, assembler ...). |
| `Test/Builder_Test_Baseline.json` | Ratchet paketlerinin taban çizgisi (`--record` ile yazılır, geçen sayı asla düşmez). |
| `Tests/` | Denetimlerin çıktı klasörü, commit'lenmez. |

## 3. Tamamlananlar (yaklaşık %24)

- Çekirdek: derleme grafiği, önbellek, VFS, Gradle/Maven, kaynak derleme,
  Dex yazma, APK/AAB paketleme, imzalama, `release`/`core`/`bridge`/`installable`
  denetimleri.
- Kotlin ve Java derleyicileri: `kotlin-omni` paketi 3790/7711 (3921 kapsam dışı, 0 hata).
- Omni IR altyapısı, `bundles` paketi 605473/605473, `filecheck` 87/90.
- TableGen: `tablegen-lit` 190/456, `tablegen-oracle` 152/459 (kapsam dışılar listeli, 0 hata); dört komut tablosu.
- Bağlayıcı: `lld-elf` 1375/2056, `lld-coff` 341/565 (0 hata).
- x86 (Claude-A, sürüyor): kodlayıcı ve AT&T/Intel yazıcı tamam; AT&T ve Intel assembler
  tamam; karıştırma/FMA/PTERNLOG/FPCLASS yorumları tamam; ELF nesne dosyası çıktısı (`X86_64.rs` içinde
  `object` modülü) tamam; `mc-x86` 598/642, kapsam içi %100 (44 kapsam dışı: COFF/Mach-O/LFI, birkaç
  seçenek ve host LLVM 18 okuyucusunun tanımadığı 5 test, gerekçeleri listeli). Disassembler (`X86_64.rs` içinde `decoder` modülü: LLVM
  X86DisassemblerTables/RecognizableInstr tabloları tablolardan üretilir, X86DisassemblerDecoder portu,
  `llvm-mc --disassemble/--hex/--mdis`) tamam; `mc-x86-disassembler` 188/188, kapsam dışı yok;
  kodla→çöz→aynı komut gidiş-dönüş testi var.

## 4. Kalan işler (öncelik sırasıyla)

1. Assembler'dan ELF/COFF nesne dosyası çıktısı (parçalar, gevşetme, düzeltmeler,
   relocation, `.eh_frame`), bizim bağlayıcıyla bağlayıp bu makinede çalıştırma testi.
2. ARMv7/ARMv8 kodlayıcı + assembler + disassembler (`Armv7.rs`, `Armv8.rs`, tablolar hazır).
3. Omni IR → makine kodu arka ucu (komut seçimi, yazmaç ayırma, çağrı kuralları) x86_64 ve ARMv8 için.
4. C/C++ ön yüzü (`Cpp.rs`) ve Rust ön yüzü (`Rust.rs`).
5. Kotlin/Java derleyicilerinin kapsamını büyütmek (`kotlin-omni` kapsam dışılarını azaltmak).
6. Uygulama (`Builder.kt`) ve yerel köprü: derleyicilerin arayüze bağlanması, cihazda uçtan uca APK.

## 5. İş bölümü (tamamlandı, kayıt olarak duruyor)

İki oturumlu dönemde amaç aynı dosyaya iki elin dokunmamasıydı. Birleştirmeden sonra bu bölüşüm
bağlayıcı değildir: ön yüz, arka uç ve Android tarafı tek elden yürür. Aşağıdaki liste yalnızca
hangi modülün hangi dönemde kimden çıktığını gösterir; tanı kodu aralıkları (`EI100–EI399` arka uç,
`EI400–EI499` ön yüz) geçerliliğini korur.

**Claude-A** (altyapı, makine kodu tarafı)
- `Compilers/Compiler_Infrastructure/` altındaki `TableGen.rs`, `Tables/`, `Tables.manifest`,
  `Elf.rs`, `Pe.rs`, `Linker.rs`, `X86.rs`, `X86_64.rs`, ileride `Armv7.rs`, `Armv8.rs`.
- `Compiler_Infrastructure.rs` (Omni IR) ve makine kodu arka ucu.
- `Compiler_Infrastructure_Diagnostic.rs` içinde `EI100–EI399` aralığı.
- Testlerde: `mod table_gen`, `mod linker_lit`, `mod x86_assembly` ve ileride ARM modülleri;
  Python'da `suites`, `differential`, `linker`, `assembler` denetimleri.

**Claude-B** (dil ön yüzleri, Android tarafı)
- `Compilers/Java/`, `Compilers/Kotlin/`, `Compilers/Cpp.rs`, `Compilers/Rust.rs`.
- `Builder/` (Kotlin uygulaması, JNI köprüsü), `Android.rs`, `Terminal.rs`, `Terminal_Speech.rs`, `Vibe_Coding.rs`.
- `Builder.rs` içindeki çekirdek modüller (paketleme, imzalama, Dex, kaynaklar, Gradle).
- `Compiler_Infrastructure_Diagnostic.rs` içinde `EI400–EI499` aralığı (yeni ön yüz kodları için).
- Testlerde kendi `mod`'ları; Python'da `core`, `release`, `bridge`, `installable`, `kotlin-omni`, `reproducible`.

**Ortak dosyalar** (ikisi de değiştirebilir, küçük ve yalnızca ekleme yapan değişikliklerle):
- `Builder.rs` içindeki `pub mod compilers` kayıt bloğu.
- `Diagnostic_Speech.rs` (yalnızca kendi cümlelerini ekle, tablolar sıralı kalsın).
- `Test/Builder_Test.rs` (yalnızca dosyanın sonuna kendi `mod`'unu ekle; başkasının modülünde
  bir yardımcıyı `pub(super)` yapmak gerekirse PR yorumunda belirt).
- `Test/Builder_Test.py`, `Test/Builder_Test_Baseline.json` (yalnızca kendi paket/denetimlerin).
- Omni IR'de (`Compiler_Infrastructure.rs`) B'nin ihtiyacı olan değişiklikler A'ya PR yorumuyla sorulur.

## 6. Derleme ve test

Ortam: Rust `1.97.1` (`rustup default 1.97.1`, `clippy` ve `rustfmt` bileşenleriyle), JDK 25
(`/usr/lib/jvm/java-25-openjdk-amd64/bin`), Python 3. Üçüncü taraf bağımlılık yok.

```sh
cargo build --lib                                   # hızlı derleme (dev profili optimize)
cargo test --lib -- --exact tests::<mod>::<test>    # tek test
python3 Test/Builder_Test.py layout comments templates dependencies strings speech independence format lint
PATH=/usr/lib/jvm/java-25-openjdk-amd64/bin:$PATH cargo test --release   # tüm testler, ~40 dk
cargo clippy --locked --all-targets -- -D warnings
cargo fmt
python3 Test/Builder_Test.py assembler            # mc-x86 ve mc-x86-disassembler ratchet paketleri
python3 Test/Builder_Test.py linker               # lld-elf, lld-coff
python3 Test/Builder_Test.py <denetim> --record   # taban çizgisini yükselt
cargo test --lib -- --ignored --exact tests::table_gen::write_tables_into_the_tree   # tabloları yeniden üret
```

Yararlı ortam değişkenleri: `APK_BUILDER_SUITE_THREADS`, `APK_BUILDER_MC_FILTER`,
`APK_BUILDER_LLD_FILTER`, `APK_BUILDER_SUITE_TRACE`. Paket ayrıntıları `Tests/suites/<paket>.txt`
ve `Tests/suites/<paket>-out-of-scope.txt` dosyalarına yazılır.

Not: `pkill -f "cargo test"` kullanma (kendi kabuğunu öldürür); `pgrep -x rustc | xargs -r kill` kullan.

## 7. Kod kuralları

1. Kaynak dosyalarda yorum ve doc-comment yok (`comments` kapısı).
2. İsimler tam kelime ve profesyonel (`Compiler_Infrastructure.rs` üslubu); kısaltma yok.
3. Testler yalnızca `Test/Builder_Test.rs` içinde, kendi `mod`'unda; Python denetimleri yalnızca `Test/Builder_Test.py` içinde. Kaynak dosyada `#[test]` yok.
4. Tanılar 14 dilde: `fail("EIxxx", "Sabit İngilizce cümle.")`; değişen veri `.with_context(format!("Etiket: {değer}"))` ile. Her yeni kod kataloğa sıralı girer; her cümle ve etiket `Diagnostic_Speech.rs` içindeki 14 dilin `*_LINES`/`*_WORDS` tablolarına sıralı eklenir.
5. Üçüncü taraf bağımlılık yok; `Cargo.toml`/`Cargo.lock` değişmez; yalnızca `std`.
6. `Compilers/Compiler_Infrastructure/` yalnızca `std`, `crate::diag`, `crate::FailureClass` ve altyapı modüllerini kullanır; `HashMap`/`HashSet`/`RandomState`/`std::env` yok. Aynı girdi her zaman aynı baytları üretir.
7. Dış araçlar (llvm-mc, llvm-tblgen, ld.lld, llvm-readobj, llvm-objdump, gcc, clang ...) yalnızca testlerde kâhin olarak kullanılır; ürün kodu hiçbir dış aracı çağırmaz.
8. Başvuru testlerinin kapsam içi kısmı %100 hedeflenir; kapsam dışı her test adıyla ve gerekçesiyle listelenir. Ratchet sayıları asla düşmez. GPL kod (GCC, binutils) çevrilmez, yalnızca davranış referansıdır.
9. Her commit'ten önce kapılar yeşil: Python kapıları, `cargo test --release`, `rustfmt`, `clippy -D warnings`.
10. Commit mesajları Türkçe ve açıklayıcı; mesaj sonunda oturumun verdiği `Co-Authored-By` ve `Claude-Session` satırları. Commit mesajında, kodda ve belgelerde model kimliği yazılmaz. (Birleştirmeden önceki commit başlıkları `[Claude-A]`/`[Claude-B]` ile başlar; yenilerinde önek yok.)

## 8. Dal, birleştirme ve yedek

- Tek çalışma dalı: `Apk_Builder_Beta` (depo `Code_Builder`). Eski depodaki `Claude-A-Test` (uç `2a58897`) ve
  `Claude-B-Test` (uç `b0cd880`) ortak ata `40df305` üzerinden birleştirildi; iki eski dal artık kullanılmıyor.
- Yeni depo, birleşik ve doğrulanmış durumun tek commit'iyle başlar. Eski deponun elde kalan kopyaları üç noktada
  sığ olduğu için (`6939f61`, `163e3c4`, `dc24302` commit'lerinin ataları hiçbir kopyada yok) eski geçmiş GitHub'a
  gönderilemedi; o geçmiş ve gerçek birleştirme commit'i bundle dosyalarında durur.
- Her push'tan sonra `.github/workflows/drive-backup.yml` dalın zip'ini Google Drive'a yükler; Drive tarafındaki
  betiğin kopyası `.github/scripts/drive_backup.gs` içindedir. Betik değişirse Apps Script'te yeni sürümle
  yeniden dağıtılır.
- Yasak: force-push, rebase, geçmişi yeniden yazmak, test silmek/atlamak.

## 9. Güncel durum

- Birleşik dal: A'nın 38 ve B'nin 14 commit'i `Apk_Builder_Beta` üzerinde tek geçmişte. Tek metin çakışması
  `Builder.rs` içindeki `project_for` idi: yerel kitaplıkların (`native_libraries`) pakete girmesi ile web
  varlıklarının (`scaffold::web_assets`) işlenmesi artık art arda çalışır.
- Claude-A: x86 assembler, disassembler ve ELF nesne dosyası çıktısı tamam: `mc-x86` 598/642 (kapsam içi
  %100, 44 kapsam dışı), `mc-x86-disassembler` 188/188; assembler çıktısı bizim bağlayıcıyla bağlanıp bu
  makinede çalışıyor. Omni IR → x86_64 arka ucu (`X86_64.rs` içinde `codegen`, çalışma zamanı Omni IR
  metniyle) ilk sürümde: `kotlin-omni-native` 3870/7711, 0 başarısız (yorumlayıcıda OK veren kutu testleri
  yerel kodda da OK). ARMv8 (`Armv8.rs`, X86_64.rs'in alt modülü) sürüyor: hedef/eşleyici genelleştirildi
  ve tablo güdümlü AArch64 kodlayıcı tamam (8871 komutun hepsi için kodlama planı); AArch64 ayrıştırıcı ve ELF
  nesne çıktısı çalışıyor (46 örnek komut ve etiketli/yeniden konumlandırmalı bir program host llvm-mc ile örtüşüyor);
  yazıcı (metin çıktısı), yönergeler (.arch/.cpu/.inst/.req...) ve sistem takma adları (dc/ic/at/tlbi) tamam;
  `mc-aarch64` (ignored test `suite_mc_aarch64`): 379 geçti, 0 başarısız, 1979 kapsam dışı (SVE/SME ayrıştırıcısı yok: +sve/+sme isteyen testler çalıştırılır, düşenler gerekçeyle kapsam dışı sayılır; big-endian/ILP32/LFI/COFF
  üçlüleri, host LLVM 18 okuyucu sınırları) ve taban çizgisine yazıldı; AArch64 disassembler (tablo güdümlü kod çözücü + Apple
  yazım türü + işaretli çıktı) `mc-aarch64-disassembler` 91/95 (0 başarısız, 4 kapsam dışı) ve taban çizgisinde.
  Eşleme sembolleri ($x/$d, `-implicit-mapsyms`), CFI kayıt adları ve yazım denetimi önerileri tamam.
  Omni IR → arm64 arka ucu ilk sürümde (`Armv8.rs` içinde `native`: derleme, bağlama ve host llvm-objdump ile çözme
  doğrulandı; test içinde yazılan AArch64 emülatörü (`mod arm_emulator`, disassembler çıktısını yorumlar) arm64 programlarını çalıştırıp
  yorumlayıcıyla aynı çıktıyı ve çıkış kodunu verdiğini doğruluyor). İki yerel arka uç da paylaşımlı kütüphane (`.so`, ET_DYN) üretiyor: x86_64 çıktısı `dlopen`/`dlsym` ile, arm64 çıktısı emülatörle çalıştırılıp doğrulandı. ARMv7 (`Armv7.rs`, ARM+Thumb+VFP/NEON/MVE): tablo güdümlü kodlayıcı, ayrıştırıcı, yazıcı, etiket/fixup, ELF32 REL, EHABI ve ARM öznitelikleri tamam; `mc-arm` 437/563 (kapsam içi 437/439, 124 kapsam dışı) ve `mc-arm-disassembler` 149/152 (kapsam içi %100, 3 kapsam dışı) taban çizgisinde. Omni IR → armv7 arka ucu (`Armv7.rs` içinde `native`, AAPCS hard-float, 64 bit değer modeli yazmaç çiftleriyle) dört yerel test programında ve paylaşımlı kütüphane testinde test içi A32 emülatörüyle yorumlayıcıyla aynı çıktıyı veriyor. Çalışma zamanı sayı biçimlendirme ve 18 matematik işlevi Omni IR olarak her üç arka uçta. Omni IR optimizer (`Compiler_Infrastructure::optimizer`): mühürlü blok yuva terfisi, iyimser sabit yayılımı, GVN, ölü parametre budama, blok birleştirme, küçük işlev açma, sembol/sabit paketleme (`strip_and_pack_symbols`, `release`), derleme parmak izi (`build_fingerprint`, FNV-1a 64), isteğe bağlı kanıt günlüğü; `checked()` her arka uçta devrede. x86-64 arka ucu: canlılık aralıkları, tek geçişli yazmaç/yuva ataması (rbx, r12-r15), sabit evleri, kaynak operandlar, birleşik karşılaştırma+dallanma, doğrudan kenar kopyaları, istisna denetimi çıkarımı, seçmeli yığın kanaryası (`ssp`); aynı makinede gcc 13 / clang ile ölçüm: tamsayı döngüsü ve bellek elek programı -O2 ile aynı seviyede, özyinelemeli fib hâlâ -O0 seviyesinde. `FunctionBuilder` artık `global_address` ve `function_address` sunar. Yapı değer geçirme için iç kural: parametre `ptr` + çağıran kopyası, dönüşte gizli ilk `sret` parametresi; dış işlevlerde yapı değeri (SysV/AAPCS sınıflandırması) henüz yok. Yeni IR işlemleri: dinamik yığın ayırma (`dynamic_stack_slot`, `stack_save`, `stack_restore`: değişken uzunluklu diziler, 4096 baytlık sayfa yoklaması, 1 GiB üstü reddi) ve değişken argümanlı işlev tanımı (`va_start`, `va_arg`, `va_copy`; 32 baytlık va_list, platform düzeninin ön eki: x86-64 SysV ve arm64 AAPCS64, ARM32 reddeder). `build_shared_library_with_imports`: tanımsız dış işlevler yükleyicinin çözeceği dinamik içe aktarma olarak kalır (gerçek glibc'ye karşı `strlen` ve değişken argümanlı `snprintf` doğrulandı). İyileştirici: işaretli/işaretsiz bölme ve kalanın sabit bölenle çarpma+kaydırmaya çevrilmesi (2'nin kuvvetleri ve Hacker's Delight sihirli sayıları, 32 ve 64 bit). arm64 arka ucu x86-64 ile aynı atayıcıyı (x19-x28) ve komut seçimini kullanır; 32 bitlik tamsayılar iki arka uçta da 32 bit biçimlerle. Rastgele program fark testleri (tamsayı ve kayan nokta; yorumlayıcı, x86-64, arm64 emülatörü) 2000'er programla uyuşuyor. Gerçek C ile ölçüm (B'nin ön yüzü, gcc 13/clang ile aynı makinede): 32 bit tamsayı döngüsü ve elek -O2 ile aynı seviyede, `%10`/`/10` döngüsü -O2'nin 1.5 katı, collatz -O0 seviyesinde, özyinelemeli fib ve kayan nokta matris çarpımı hâlâ -O0 civarında. Kayan nokta değerleri artık yazmaçta (x86-64 xmm8-xmm15, arm64 d24-d31; çağrıdan sağ çıkmayanlar) ve NaN yalnızca değer aritmetikten çıktığı yerde kanonikleştirilir. İyileştirici: sabit parametreli dallanmaların atlatılması (jump threading), taze tahsis için null/instance_of katlama, üç yollu karşılaştırmanın tek karşılaştırmaya inmesi, ima edilen dallanma, dizi uzunluğu GVN, sınıf başlatma korumasının hızlı/yavaş yola bölünüp satır içine açılması. `mc-aarch64` 1161/2358 (0 başarısız; SVE yazmaç, yüklem, vektör listesi, desen ve genişletmeli adresleme ayrıştırması eklendi, kalan SVE/SME çoğunlukla tanı iletileri) ve `mc-aarch64-disassembler` 92/95. `String.format` çalışma zamanı (`omni/runtime/Formatting.java`; JDK ile 20803 durumda aynı). Gerçek proje testleri: üç dosyalı x86-64 Assembly projesi, beş Kotlin/Java projesi (JVM, yorumlayıcı, x86-64 ve arm64 emülatörü aynı çıktı) ve ağır hesap projesi (1M sıralama, 5M elek, 1M collatz, matris: yerel kod JVM JIT'ine yakın, küçük programlarda başlangıç 10 kat hızlı). Güvenlik: yığın kanaryası (`ssp`) x86-64 ve arm64'te, paylaşımlı kitaplıklarda `.init_array` yapıcısıyla `getrandom`'dan yükleme anında tohumlanır; arm64 işlevleri dönüş adresini `paciasp`/`autiasp` ile imzalar. `mc-aarch64` 1231/2358 (SME: ZA, döşeme, dilim, `zt0`; compiler-rt `sme-abi.S` ve SVE `memcpy`/`memset` derleniyor). İyileştirici işlevleri iş parçacıklarında çalıştırır (sıra belirleyici). Çalışma zamanı: `String.format`, `int[]`/`long[]` introsort, onluk `Long.toString` ve `StringBuilder.append(int/long)` hızlı yolları. Gerçek proje testleri sekiz Kotlin/Java projesinde (envanter, metin, özellikler, ağır hesap, biçimleme, sıralama, sayılar, karışık Java) JVM, yorumlayıcı, x86-64 ve arm64 emülatörünün çıktısını birebir karşılaştırır; ön yüz hataları B'ye yazıldı (`compareBy` ve `Long += Int?` kalıpları). Sırada: döngü değişmezi taşıma ve sınır denetimi eleme, dış ABI sınıflandırması, thread-local, kalan SVE/SME tanıları, big-endian/ILP32.
- Claude-A (web, kullanıcı onayıyla `Compilers/Html.rs` ve `Compilers/Js.rs`): JavaScript ES2023 sözcük çözümleyici, ayrıştırıcı ve erken hata denetimi (`js::parse_script/parse_module/parse`, ESTree biçimli `Program::to_json`, kaynak yazıcı `js::generate`; acorn AST'siyle 9080 gerçek dosyada betik ve modül kipinde fark yok, gidiş-dönüş denetimi geçiyor; test262 `js-test262` 23722/23722; using, import defer/source, dekoratörler ve düzenli ifade değiştiricileri dahil; `js-roundtrip` 19271 geçti, 0 başarısız). HTML: WHATWG sözcük çözümleyici (tüm durumlar, 2231 adlandırılmış karakter başvurusu) ve ağaç kurucu (ekleme kipleri, benimseme ajansı, yabancı içerik, şablon, yarı yarıya parçalı ayrıştırma bağlamı, yeni `select` kuralları): `html::parse_document/parse_fragment_in/parse_strict`, `serialize`, `dump` (html5lib biçimi), `extract_scripts/extract_styles/extract_inline_styles`, `check_scripts`, `check_embedded` (stil ve betikleri `css::` ve `ts::` okuyucularına verir). `html5lib-tree` 1692/1692 ve `html5lib-tokenizer` 6802/6806 (4 test yalnız vekil içeriyor; betik çalıştıran `scripted/` testleri JS çalıştırıcı olmadığı için alınmadı), parse5 ile 17701 gerçek dosyada fark yalnızca BOM, `feDropShadow` ve yeni `select` kuralları. `plugin.html` ve `plugin.js` derleyici sözleşmesiyle çalışır (kaynağı okur, denetler, kanonik biçimde yazar). Tanı kodları: `EI720`/`EI721` HTML, `EI770`/`EI771` JS.
- Claude-B: C ön yüzü (`Compilers/Cpp.rs` içinde C kısmı): önişlemci, sözcük çözümleyici, ayrıştırıcı,
  anlam çözümlemesi ve Omni IR'ye indirgeme. Ölçü: Clang'ın `test/Preprocessor`, `test/Lexer`, `test/Parser`,
  `test/Sema` lit testlerinden C olanları, yeni ratchet paketleri (`clang-c-*`), kapsam dışılar gerekçeli listeli.
  Yeni tanı kodları `EI400–EI499`, testler `mod c_front_end`.
  Güncel ölçü: `clang-c-lexer` 39/141, `clang-c-parser` 30/425, `clang-c-preprocessor` 161/395 (0 başarısız),
  `clang-c-sema` 115/1382. Kalan sema başarısızlıkları 14 test; büyük kapsam dışı kalemler: GNU öznitelikleri
  (~190 test), yazım düzeltme (~95), yerleşik kütüphane işlevi tablosunun sema tarafı (~80; `__has_builtin` artık Clang'ın `.td` tablolarından üretilmiş sözcük listesiyle hedef ve dile göre yanıtlıyor), `-fblocks`, macho/coff üçlüleri.
- Claude-B, C → Omni IR indirgemesi (PR #6, `Claude-B-Test`): `FunctionBuilder`'ın mühürlü blok SSA'sıyla
  (adresi alınanlar `StackSlot`) sabitler/aritmetik, yerel değişkenler, atama/if-else/`&&`/`||`/`?:`, döngüler,
  `switch` (GNU `case a ... b` dahil), fonksiyon çağrıları (özyineleme ve işlev işaretçisi üzerinden dolaylı
  çağrı dahil), dizi/struct/işaretçi aritmetiği, `goto`/etiketler, zararsız GNU öznitelikleri (yerleşimi değiştirenler gerekçeli reddedilir), değişken uzunluklu diziler ve `__builtin_alloca` (öbekte ayrılıp işlev çıkışında serbest bırakılır; IR'da dinamik yığın işlemi gelince değiştirilebilir), bileşik sabitler (`(struct S){...}`, `(int[]){...}`, dosya kapsamı dahil), yapı ataması ve değerle geçirme/döndürme (A'nın iç çağrı kuralı: aggregate parametre `ptr` + çağıranın kopyası, dönüş gizli ilk `sret` parametresi), kütüphane işlevlerini (`printf` gibi, değişken sayıda argümanlı `Import` dahil) bildirme/çağırma, bit alanı okuma/yazma, dize sabitleri,
  başlatıcı listeleri (tasarımlı başlatıcılar, union ve bit alanlı struct dahil), float/double,
  dosya kapsamı ve işlev içi `static` globaller (başlatıcılı/başlatıcısız, `extern`, işaretçi yeniden
  konumlandırmaları, dize sabitleri) indirgeniyor. Her `lowering_*` testi hem yorumlayıcıya hem x86_64 native
  çıktıya karşı koşuyor. Desteklenmeyen her şey (şu an: dış işlevlerde yapı değeri (SysV/AAPCS sınıflandırması A'da), değişken sayıda argümanlı işlev tanımları (`va_start`/`va_arg` IR işlemi gerekiyor), çok boyutlu VLA, `__builtin_*` çağrılarının küratörlü kümesi dışında kalanlar (`expect`, `unreachable`, `trap`, `memcpy`/`memmove`/`memset`, `popcount`/`clz`/`ctz`/`ffs`/`parity`, `bswap`, `abs`, `fabs`/`sqrt`/`floor`/`ceil`, `huge_val`/`inf` desteklenir), iç `extern`, thread-local)
  sessizce atlanmıyor; o fonksiyon modüle girmiyor, nedeni `LoweredUnit::gaps` içinde bildiriliyor.
  Linkage: `static` → `Internal`, diğerleri → `Export`, tanımsız `extern` global → `Import`.
  Gerçek proje: zlib (`v8/third_party/zlib`, 10 birim, 125 işlev) bu ön yüzle derlenip yorumlayıcı, x86_64 native ve ARM64 emülatöründe gcc'nin ürettiği baytlarla birebir aynı çıktıyı veriyor (`mod real_projects`); GCC libiberty'nin 6500 satırlık C++ isim çözücüsü (`cp-demangle.c`) 57 sembolü gcc derlemesiyle aynı çözüyor (`mod real_demangler`). Her ikisi yorumlayıcı, x86_64 native ve ARM64 emülatöründe koşuyor.
  Java gerçek testleri (`mod real_java`): Kotlin deposundaki `MavenComparableVersion.java` ve 228 üyeli aşırı yüklemeli `DataTypeClass.java` bizim Java derleyicisiyle derlenip `javac` derlemesiyle aynı çıktıyı veriyor (yansıtma sürücüsü dahil). Assembly gerçek testleri (`mod real_assembly`): compiler-rt'nin 534 dosyasından x86_64 (4/4), i386 (16/16), ARM (127/127), AArch64 (4/6; `lse.S` şablon, `sme-abi.S` assembler boşluğu) `.S` dosyası önişlemcimiz + A'nın assembler'ından geçiyor; ARM tamsayı yardımcıları ARM32 emülatöründe, x86_64 `floatundidf/sf` C ön yüzümüzün çıktısına bağlanıp native çalışıyor.
  Ürün girişi: `cpp::compile_c(kaynaklar, başlıklar, argümanlar)` birden çok C birimini derleyip tek Omni modülünde birleştirir
  (EI400 kaynak hatası, EI401 desteklenmeyen yapı, EI402 yinelenen sembol); ABI/hedef seçimi çağıranın işi.
- Claude-B, gerçek proje hazırlığı (PR #6): C++ ön yüzü yazılmayacak; C, Java, Kotlin ve Assembly gerçek GitHub projeleriyle
  sınanıyor. Yöntem: depoyu klonla → `workspace::adopt_folder` ile Gradle yapısını benimse → `maven::plan` ile bağımlılıkları indir →
  `apk_builder_build_project` ile APK üret; her hata sınıfı kalıcı bir test olarak (`mod java_real_world`, `mod kotlin_real_world`,
  `mod dex_*`) kaydedilir. Ayrıştırma taraması (25 depo): Kotlin 5369/5382, Java 1811/1833 (kalan 22 dosya `module-info.java`; derlemede atlanır).
  Uçtan uca APK'ya derlenenler: KISS (Java, ~500 sınıf) ve Fossify Notes (Kotlin, AndroidX, Compose kütüphaneli, 38 MB). Compose
  derleyici eklentisi gerektiren uygulamalar (Fossify Calculator gibi `@Composable` yazanlar) bilerek reddedilir. Bu turda eklenenler:
  Gradle `platform()` (BOM), `maven { url }` depoları (`Builder.json` `repositories`), `project.property("X")`, eski `com.android.support`
  kütüphanelerinin AndroidX varken çıkarılması, kaynak tablosunda dizinli yineleme denetimi ve 2 milyon kayıt sınırı; Java'da genel üst
  sınıftan miras alınan alan/yöntem türleri ve lambda içindeki yerel sabitler; Kotlin'de Java statik başvuruları, akıllı dönüşümler,
  `synchronized`, SAM dönüşümü; Dex'te 255'ten çok kayıt gereken yöntem (parametreler geldiği yerde okunur), 32K'dan uzak dallar
  (`goto/32`) ve sığmayan kütüphane yöntemleri için W6004 uyarısı.
- Web dilleri (HTML, CSS, JS, TS; Rust ve Assembly ile sıfırdan): Claude-A HTML ve JS'i, Claude-B CSS ve TS'i alır. Her dil kendi dosyasında:
  `Compilers/Html.rs`, `Compilers/Js.rs` (A), `Compilers/Css.rs`, `Compilers/Ts.rs` (B). Tanı aralıkları: CSS `EI700–EI749`, TS `EI750–EI799`
  (HTML/JS için A ayırır). Sözleşmeler: `css::parse_stylesheet`, `css::parse_declarations`, `ts::transpile` (TS → JS metni; çıktı A'nın JS
  ayrıştırıcısıyla doğrulanır). Testler `mod css` ve `mod typescript` içinde; `Compilers/*.rs` içinde yorum satırı yok.
- Claude-B, C ön yüzü gerçek projelerle (PR #6): Lua (1200 işlev), cJSON, miniz, bzip2, lodepng, zlib, inih, stb_truetype ve zstd'nin büyük
  kısmı `compile_c` ile Omni IR'ye derleniyor. Eklenenler: GNU deyim ifadeleri, `__builtin_offsetof`, `packed`, `volatile`, NaN ve
  sınıflandırma builtin'leri, iç içe `case`, `weak` tanımlar, birimler arası struct geçişi ve variadic işlevler (`va_start/va_arg/va_copy`;
  proje içinde 8 baytlık dizi sözleşmesiyle, x86_64 ve AArch64 `va_list` düzeniyle uyumlu; `printf` ailesi gerçek ABI'de kalır).
  Kalanlar (IR'da karşılığı yok, A ile konuşulacak): hesaplanmış `goto`, 32-bit hedefte variadic, `vector_size`, `long double`, TLS.
- Claude-B, web varlıkları (PR #6): `scaffold::web_assets` Assets klasöründeki `.ts` dosyalarını `.js` olarak yazar (`.d.ts` atılır, TS
  hatası derlemeyi durdurur), `.css` dosyaları doğrulanır ve sorunlar uyarı olarak bildirilir. HTML/JS için aynı noktaya A'nın adımı
  eklenebilir.
- Claude-B, MaterialFiles ile bulunan Kotlin/Gradle düzeltmeleri: `dependencies {}` içindeki `def` yerelleri, `resValue` birleştirmeleri,
  `version { strictly }` blokları, POM'da sürümsüz bağımlılıklar, `.kotlin_module` ile dosya cepheleri, sözleşmeli işlevlerden akıllı
  dönüşüm, `?:` ile bitirilen `as?`, `if` kolları arasında sızan akıllı dönüşüm, vararg lambda çıkarımı, iç içe lambda dönüş türleri,
  `?.` ile atama, birincil/ikincil kurucu eşleştirmesi, Java'da tek tür içe aktarımının aynı paketten önce gelmesi.

## 10. Gerçek proje sicili (iki oturum da burayı okur ve günceller; aynı projeyi ikinci kez sınamayın)

Her satır: proje, kim sınadı, ne doğrulandı, sonuç. Yeni bir projeye başlamadan önce burada ve PR #5/#6'daki son yorumlarda ara.

| Proje | Dil | Kim | Doğrulanan | Sonuç |
|---|---|---|---|---|
| zlib (`v8/third_party/zlib`) | C | B | derle + yorumlayıcı/x86-64/arm64 çıktı gcc ile aynı | geçer (`mod real_projects`) |
| GCC libiberty demangler (`cp-demangle.c`) | C | B | 57 sembol gcc ile aynı, üç motor | geçer (`mod real_demangler`) |
| stb (`stb_image`, `stb_image_write`, `stb_ds`, `stb_perlin`, `stb_dxt`, `stb_rect_pack`, `stb_divide`; 328 işlev) | C | A | PNG kodla/çöz, karma tablo/dizi/dize haritası, gürültü, DXT, dikdörtgen paketleme; çıktı gcc ile aynı (`-DSTBI_NO_THREAD_LOCALS`; iş parçacığı yerel değişkenleri B'nin bilinen boşluğu) | geçer; `unsequenced` çözümlemesi tanımsız davranışı doğru yakaladı |
| Lua 5.4 (tüm `l*.c` + `lua.c`, 1244 işlev) | C | A | paylaşımlı kitaplık + gerçek glibc; `test.lua` çıktısı gcc derlemesiyle bayt bayt aynı (`-DLUA_USE_JUMPTABLE=0`) | geçer; hız: gcc -O0'a 1,1-1,5x (atlama tablosu öncesi 2,5-3x) |
| cJSON | C | A | ayrıştır/yazdır/değiştir çıktısı gcc ile aynı | geçer |
| miniz (deflate, inflate, zip yazma/okuma) | C | A | 300 KB girdi, 4 düzey, zip arşivi ve CRC'ler gcc ile aynı (`-DMINIZ_NO_STDIO -DMINIZ_NO_TIME`) | geçer |
| lz4 1.11 (`lz4.c`, `lz4hc.c`, `lz4frame.c`, `xxhash.c`) | C | A | hızlı, HC 3/6/9/12, çerçeve biçimi; sıkıştırılmış baytların özeti gcc ile aynı | geçer (379 işlev) |
| bzip2 1.1 (kitaplık) | C | A | 500 KB girdi, düzey 1/5/9, sıkıştır + aç, özet gcc ile aynı | geçer (54 işlev) |
| lodepng | C | A | PNG kodla/çöz (32/24 bit, otomatik renk türü); baytlar gcc ile aynı | geçer (245 işlev) |
| zstd 1.6 (`common`, `compress`, `decompress`; 5090 işlev) | C | A | düzey 1/3/9/19 sıkıştırılmış baytlar ve açılmış veri gcc ile aynı; hız gcc -O0'dan hızlı (0,74 s'ye karşı 0,92 s) | geçer, ama ön yüz boşlukları nedeniyle kaynakta yamalar gerekti (aşağıda) ve `-DZSTD_NO_TRACE` |
| compiler-rt `.S` (534 dosya) | Assembly | B | önişlemci + A'nın assembler'ı | x86_64 4/4, i386 16/16, ARM 127/127, AArch64 5/6 |
| MavenComparableVersion.java, DataTypeClass.java | Java | B | javac ile aynı çıktı | geçer (`mod real_java`) |
| KISS (Java, ~500 sınıf) | Java/Android | B | uçtan uca APK | geçer |
| Fossify Notes | Kotlin/Android | B | uçtan uca APK (38 MB) | geçer |
| NewPipe | Kotlin/Java/Gradle | B | hazırlık (buildSrc, BOM, depolar) | sürüyor |
| 8 küçük Kotlin/Java projesi (envanter, metin, özellikler, ağır hesap, biçimleme, sıralama, sayılar, karışık Java) | Kotlin/Java | A | JVM = yorumlayıcı = x86-64 = arm64 emülatörü | geçer (`real_kotlin_and_java_projects_agree...`) |
| 3 dosyalı x86-64 Assembly projesi (sieve + yazdırma) | Assembly | A | A'nın assembler + bağlayıcısı, çalışır | geçer |
| mini-gmp (GMP 6.3 `mini-gmp.c`, 4600 satır bignum kitaplığı) | C | A | derle + paylaşımlı kitaplık olarak gerçek glibc ile yüklenip gcc'nin hesapladığı değerleri üretiyor (100!, powm, sqrt, gcd, fib, asallık) | geçer (`mod real_bignum`); bulunan arka uç hatası: içe aktarılan veri globalleri (`stderr`) GOT ile alınmalıydı, düzeltildi |
| GCC libiberty: md5, hashtab, fibheap, GNU regex (157 KB `regex.c`), xmalloc | C | A | paylaşımlı kitaplık + gerçek glibc; md5 özetleri, tablo/yığın toplamları ve 6 düzenli ifade eşleşmesi gcc ile aynı | geçer (`mod real_libiberty`); libiberty cp-demangle B'de |
| compiler-rt `builtins` birim testleri (`test/builtins/Unit`, 226 test) | C | A | gerçek `lib/builtins/*.c` + test, x86-64 yerel ve arm64 emülatörü | 104 geçer iki motorda da, ayrışma yok; kalan ön yüz boşluğu: `__builtin_sadd_overflow` (15), long double/`xf_float` (14), `ti_int` (128 bit), karmaşık başlatıcılar (paketler `compiler-rt-builtins-x86/arm64`) |
| GCC `gcc.c-torture/execute` (1775 kendi kendini denetleyen C programı) | C | A | derle + x86-64 yerel çalıştır; ayrıca arm64 emülatörü | 453 geçer, 18 çalışma hatası (ön yüz), 1304 ön yüz boşluğu (`mod torture`, paketler `gcc-torture-x86/arm64`) |
| `kotlin-omni` ve `kotlin-omni-native` (Kotlin kutu testleri, JVM karşılaştırmalı) | Kotlin | A (B'nin `7041c2a` dalında) | JVM = Omni yorumlayıcı = yerel x86-64 | 3826 ve 3908 geçer, 0 başarısız; baseline 3790 ve 3870 (+36, +38); yerelde ~2 saat |
| 128 bitlik tamsayılar (`wide::legalize`: her `i128` iki `i64`) | IR | A | yorumlayıcı = x86-64 = arm64 = arm32; 12 değer × toplama, çarpma, kaydırma, bölme, karşılaştırma, `select`, dönüşüm, yük/saklama, çağrı | geçer (`wide_integers_agree_...`); yok: kayan noktaya dönüşüm, `popcount/clz/ctz/bswap`, dönme, `switch` |
| SQLite 3.46 amalgamation (`sqlite3.c`, 250 bin satır) | C | A | gcc ile aynı SQL betiği çıktısı hedefi; gerçek glibc başlıklarıyla (`APK_BUILDER_C_INCLUDE=/usr/include:<gcc include>`) ayrıştırma ve işlev indirgeme | **B'de bekliyor**: `__atomic_load_n/__atomic_store_n` (19 yer), `yy_reduce`: "statements before the first switch label", `unixFileLock/unixRemapfile/unixCheckReservedLock`: "calls through variadic function pointers", `long double` yerelleri (`-DLONGDOUBLE_TYPE=double` ile aşılabilir) |
| benchmarks-game tarzı Kotlin programları: n-body (1000 adım), spectral-norm (n=100), fannkuch-redux (n=7), binary-trees (derinlik 10, öbek baskısı), mandelbrot (96x96) | Kotlin | A | JVM = yorumlayıcı = yerel x86-64 (+ arm64 emülatörü); bilinen sonuçlar (enerji -0.169075164 / -0.169087605, 1.274219991, `228 16`, 4095 / 2047) | geçer (`real_kotlin_and_java_projects_...`, 13 proje); küçük iş yükünde yerel kod JVM'in başlatma süresinden hızlı |
| musl libc x86-64 Assembly (31 `.s` dosyası: `memcpy`, `memmove`, `memset`, `setjmp`, `longjmp`, x87 `floorl/expl/logl/exp2l/log2l`, `fenv`, `clone`, `vfork`...) | Assembly | A | 31 dosyanın tamamı A'nın assembler'ından hatasız geçiyor; 12'si bağlanıp 130 satırlık bir Assembly sürücüyle çalıştırılıyor: boyut 0-300 aralığında bellek rutinleri sınır korumalarıyla, çakışan `memmove` yönleri, `setjmp/longjmp`, x87 yuvarlama ve üstel/logaritma | geçer (`mod real_musl_assembly`, `APK_BUILDER_MUSL_ROOT=<musl>` ile) |
| expat 2.x (`xmlparse.c`, `xmltok.c`, `xmlrole.c`; 445 işlev) | C | A | ad alanlı belge, DTD varlığı, CDATA, yorum; 801 öğe, 800 öznitelik, özet; hatalı belgede hata kodu ve satır/sütun gcc ile aynı | geçer (`expat_config.h` elle yazıldı) |
| Brotli (`common`, `dec`, `enc`; 3871 işlev) | C | A | 300 KB girdi, kalite 0/4/9/11, sıkıştırılmış baytların özeti ve açılmış veri gcc ile aynı | geçer; SSE yolları kapatıldı (`-U__SSE2__` vb.), `-DBROTLI_MODEL(M)=`, `__builtin_constant_p` ve `__builtin_prefetch` makroyla susturuldu (B'ye bildirilen boşluk) |
| jansson 2.x | C | A | JSON oluştur/yaz/oku/karşılaştır | **B'de bekliyor**: `hashtable_seed.c`, `strconv.c`: "calls to a function without a fixed prototype" (K&R/prototipsiz çağrı, B'nin 1 numaralı maddesi) |
| PCRE2 10.x (UTF-8 + Unicode özellikleri, 215 işlev) | C | A | 8 desen (adlı gruplar, ileri bakış, `\\p{Lu}`, tembel nicelik, büyük/küçük harf duyarsız): tüm eşleşme aralıkları ve özet gcc ile aynı | geçer (JIT kapalı; `config.h`, `pcre2.h`, `pcre2_chartables.c` genel şablonlardan) |
| mbedTLS 4.x | C | A | — | **atlandı**: kriptografi `tf-psa-crypto` alt modülünde, sığ klonda yok |
| libpng + zlib (701 işlev) | C | A | Adam7 serpiştirmeli RGBA yazma ve okuma, bellekten G/Ç geri çağrıları, kesik akışta `png_error` → `longjmp` ile hata yakalama; PNG baytlarının özeti gcc ile aynı | geçer; SIMD yolları kapatıldı (`-U__SSE2__`), `fcntl.h` yalnızca bildirim saplaması |
| ARM optimized-routines (34 aarch64 `.S`: `memcpy`, `memmove`, `memset`, `memchr`, `strlen`, `strcmp`, `strcpy`... SVE, SVE2, MTE, MOPS varyantları dahil) | Assembly | A | ön işlemci + A'nın aarch64 assembler'ı; 34 dosyanın tamamı hatasız nesneye çevriliyor | geçer (`mod real_arm_routines`, `APK_BUILDER_ARM_ROUTINES_ROOT=<kök>` ile); koşturma yok: emülatör NEON/SVE komutlarını yürütmüyor |
| LZMA SDK (C; `LzmaEnc`, `LzmaDec`, `LzFind`, `7zCrc`) | C | A | LZMA sıkıştırma/açma, düzey 1/5/9 | **B'de bekliyor**: `7zCrc.c`: "calls to a function without a fixed prototype" (K&R), `CpuArch.c`: `asm` (cpuid). gcc referansı hazır (özet `883e9709...`, `a611c770...`) |

zstd için B'ye ön yüz boşlukları: gerçek `__asm__` (`ZSTD_selectAddr` cmov, `cpuid`, bariyer olarak `__asm__("" : "+r"(x))` ve hizalama `__asm__(".p2align 6")`), `__builtin_prefetch` (etkisiz ipucu olarak kabul edilmeli), zayıf tanımsız sembol (`__attribute__((weak))` bildirimi çağrılmadan önce `if (f)` ile sınanıyor; içe aktarma zayıf olmalı).

Sahiplik: C/Kotlin/Java ön yüz boşlukları B'nin, arka uç/iyileştirici/assembler/bağlayıcı hataları A'nın. Bir test düşünce nedeni sınıflandırılır ve ilgili tarafa yazılır.

## 11. Arka uç destek durumu (A) ve bilinen boşluklar

| Konu | Durum |
|---|---|
| 128 bitlik tamsayı (`i128`) | **Hazır**, x86-64, arm64 ve arm32: `compiler_infrastructure::wide::legalize` her `i128` değerini iki `i64` yarıya böler. Toplama, çıkarma, çarpma, bölme/kalan (işaretli ve işaretsiz), kaydırma, dönme, karşılaştırma, `select`, `min/max`, `abs`, `popcount/clz/ctz/bswap/bitreverse`, `zext/sext/trunc`, kayan noktaya ve kayan noktadan dönüşüm (yuvarlama ve doygun varyantlar), yük/saklama, çağrı ve dönüş. Yoksa: `smulh/umulh`, atomikler, `switch`, `Invoke`. Dışa açık `i128` imzaları harici ABI'ye uymaz (iki `i64` + gizli sonuç işaretçisi). |
| Thread-local globaller | **x86-64'te hazır**: `.tdata`/`.tbss`, başlangıç-çalışma (IE) erişimi; çalıştırılabilirde `_start` iş parçacığı bloğunu kurar (tek iş parçacığı), glibc'ye yüklenen kitaplıkta gerçek iş parçacığı başına depolama (testli). arm64 ve arm32'de **boşluk** (`thread-local globals`). |
| Hesaplanmış `goto` | IR'a yeni terminatör **gerekmiyor**: ön yüz `&&etiket`'i etiket sırasını veren küçük bir tam sayıya, `goto *p`'yi bu sayı üzerinde `switch`'e indirgemeli. x86-64 `switch` yoğun durumda atlama tablosuyla çalışır. |
| Dış işleve struct değeri | **Boşluk (tasarım gerekli)**: ≤16 baytlık argümanlar ön yüzde skaler parçalara bölünerek bugün de geçirilebilir; ≤16 baytlık dönüş (`rax:rdx`) ve >16 bayt (`byval`) için IR'da çok değerli dönüş ve `byval` bayrağı gerekir. |
| Vektörler (`vector_size`, SSE/NEON içsel işlevleri) | **Boşluk**: arka uçlar `Type::Vector` değerlerini reddediyor. Ön yüzde skaler alternatif yolları kapatmak (`-U__SSE2__`) bugün çalışan çözüm. |
| `long double` (x87) | **Boşluk** (kasıtlı): `double` olarak ele alınmalı ya da reddedilmeli. |
| arm64 `switch` | Doğrusal karşılaştırma zinciri (x86-64'teki atlama tablosu yok): hız işi. |


## 12. Hız analizi (A): ölçülen boşluk, gcc kapatma deneyi, denenip tutmayanlar

zstd (60 dosya, çıkarma kıyası gcc ile aynı), makine ±%10 oynar; kesin sayılar callgrind komut sayısıdır.

| Derleyici | Süre | Komut sayısı |
|---|---|---|
| gcc 13 `-O0` | 1,02 sn | ölçülmedi |
| Apk Builder x86-64 (f69245a) | 0,49 sn | 5,86 G |
| gcc `-O1` / `-O2` / `-O3` | 0,284 / 0,29 / 0,296 sn | 2,81 G (`-O2`) |
| gcc `-O2 -fno-inline` | 0,61 sn | ölçülmedi |

Komut sayısı boşluğunun (3,05 G) dağılımı (yığın erişimi, gcc'de `rsp`/`rbp` göreli): yığından yükleme 1,05 G (gcc 0,17 G), yığına yazma 0,76 G (gcc 0,06 G), yazmaçtan yazmaça 0,57 G (gcc 0,25 G), `jmp` 0,23 G (gcc 0,05 G). Yığın trafiği tek başına boşluğun yaklaşık yarısı.

gcc `-O2` ile tek geçiş kapatma (zstd süresi, taban 0,280 sn): inline kapalı +%118, `schedule-insns2` +%7, `ccp`/`copy-prop` +%7, `loop-im` (LICM) +%6, `strict-aliasing` +%6, `dominator-opts`/`fre` +%5, `ivopts` +%3, `ipa-cp`/`ipa-sra` +%3, `sra` +%2, `pre`/`gcse` +%1, `if-conversion` ve `reorder-blocks` 0. `-O3`, `-O1`'den farksız: zstd'de kazanç inline ve kayıt tahsisi kalitesinden geliyor, tek bir iyileştirici geçişten değil.

Denenip tutmayanlar (ölçüldü, geri alındı): döngü sınırında canlı aralık bölme (IR'da blok parametresiyle; zstd 7,887 G → 7,891 G ve → 8,19 G), `inline` ipuçlu işlevlerde sınırı 400'den 6000'e çıkarma (komut sayısı değişmedi, derleme 4 sn → 12 sn), `StackSlot` içeren işlevlerin inline'a açılması (−%2,4 komut, süre aynı, derleme 40 sn), sabit bağımsız değişkenli çağrı noktasında büyük işlev inline'ı (−%0,4 komut, derleme 20 sn).

İyileştiricide olmayanlar (gcc'de olan): LICM, indüksiyon değişkeni güçlendirme (ivopts), döngü döndürme/açma, bellek üzerinden yük/yaz iletme ve ölü yazma silme (takma ad analizi yok), PRE, SROA (yalnız skaler `promote_slots` var), IPA sabit yayılımı/klonlama, kuyruk çağrısı, dal olasılığı ve profil. Arka uçta olmayanlar: canlı aralık bölme, bellekten yeniden üretme (rematerialization), komut zamanlama.

Ek ölçümler (A, aynı zstd düzeneği): gcc `-O2`'yi yazmaç kısıtlayarak derledim. 8 yazmaç kapalı (`-ffixed-r8..r15`) 0,303 sn, 10 yazmaç kapalı (yalnızca ≈4 kullanılabilir) 0,328 sn; taban 0,276 sn. gcc 4 yazmaçla bile bizim 12 yazmaçlı 0,48 sn'mizden hızlı; yani fark yazmaç sayısında değil, kodun hacminde. Bizim yazmaç sayısını 7'den 3'e indirmek komutları yalnızca +%1,6 artırıyor (yazmaç başına ≈%0,4).

Statik kod hacmi (komut sayısı): `opt_generic` bizde 2142 makine komutu, gcc'nin `ZSTD_compressBlock_opt2`'si 1004; bizim IR'ımız (iyileştirme sonrası) 1441 komut, yani gcc'nin nihai makine kodundan fazla. Kontrol akışı: `branch` 107 + `jump` 94 (gcc ≈50 + 29), `icmp` 108 (gcc 53 `cmp` + 17 `test`). `ZSTD_compressBlock_fast` bizde 53 KB, gcc'de 15 KB.

Sıcak iç döngü (ikili ağaç araması) iterasyon başına: bizde 105 komut ve 73 yığın erişimi, gcc'de 52 komut ve ≈9 yığın erişimi. Geri kenarda ≈10 döngü-taşınan değer yığın yuvasından yığın yuvasına kopyalanıyor (`Q = P` yalnız bir yolda; iki değer aynı anda canlı olduğu için yuva paylaşılamıyor).

Denenip tutmayanlar (ek): `StackSlot` adreslerini yeniden üretilebilir ev yapmak (−%0,1), spilled adres ve karşılaştırmaları kullanımda yeniden hesaplamak (A/B süre farkı yok), çok kullanımlı `offset`'leri bellek işlenenine gömmek (−%0,5 zstd, +%1,2 Lua), blok içi/EBB yük-yaz iletme (−%0,08; takma ad analizi tür bilgisi olmadan çoğu yükü silemiyor, gcc'de `strict-aliasing` +%6).

Sonuç: tek bir mekanizma eksik değil; gcc'nin kazancı çok sayıda küçük geçişten (her biri %1–7) ve inline'dan geliyor, bizim IR'ımız ise ön yüzden gelen ayrıntı yüzünden gcc'nin makine kodundan büyük. Sıradaki somut işler: (1) ön yüzden tür tabanlı takma ad bilgisi (TBAA) ve yük/yaz iletme, (2) LICM, (3) SCCP/kopya yayılımı kapsamını genişletmek, (4) ön yüz IR'ında koşul zincirlerini (`&&`, `||`) doğrudan dal olarak üretmek.
