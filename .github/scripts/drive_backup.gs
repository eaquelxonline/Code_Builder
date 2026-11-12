const REPO = 'Code_Builder';
const BRANCH = 'Apk_Builder_Beta';
const FOLDER = 'Code_Builder_Yedek';
const PREFIX = REPO + '_' + BRANCH + '_';
const COPTE_KALAN = 2;
const DRIVE = 'https://www.googleapis.com/drive/v3/files';
const DRIVE_YUKLEME = 'https://www.googleapis.com/upload/drive/v3/files?uploadType=resumable';

function ozellik_(ad) {
  const deger = PropertiesService.getScriptProperties().getProperty(ad);
  if (!deger) throw new Error(ad + ' betik özelliği tanımlı değil (Proje Ayarları → Betik Özellikleri)');
  return deger;
}

function yetki_() {
  return { Authorization: 'Bearer ' + ScriptApp.getOAuthToken() };
}

function klasor_() {
  const bulunan = DriveApp.getFoldersByName(FOLDER);
  return bulunan.hasNext() ? bulunan.next() : DriveApp.createFolder(FOLDER);
}

function yedekler_(klasor) {
  const bulunan = [];
  const dosyalar = klasor.getFiles();
  while (dosyalar.hasNext()) {
    const dosya = dosyalar.next();
    if (dosya.getName().indexOf(PREFIX) === 0) bulunan.push(dosya);
  }
  return bulunan;
}

function commit_(sha) {
  if (!/^[0-9a-f]{40}$/.test(sha || '')) throw new Error('sha geçersiz');
  return sha.slice(0, 7);
}

function boyut_(boyut) {
  if (!/^[1-9][0-9]{2,11}$/.test(boyut || '')) throw new Error('boyut geçersiz');
  return Number(boyut);
}

function durum_(sha) {
  const kisa = commit_(sha);
  const yedekli = yedekler_(klasor_()).some(function (dosya) {
    return dosya.getName().indexOf(PREFIX + kisa + '_') === 0;
  });
  return yedekli ? 'güncel: ' + kisa + ' zaten yedekli' : 'eksik: ' + kisa + ' yedekli değil';
}

function baslat_(sha, boyut) {
  const kisa = commit_(sha);
  const bayt = boyut_(boyut);
  const ad = PREFIX + kisa + '_' + Utilities.formatDate(new Date(), 'UTC', 'yyyyMMdd-HHmm') + '.zip';
  const yanit = UrlFetchApp.fetch(DRIVE_YUKLEME, {
    method: 'post',
    contentType: 'application/json; charset=UTF-8',
    headers: Object.assign(yetki_(), {
      'X-Upload-Content-Type': 'application/zip',
      'X-Upload-Content-Length': String(bayt),
    }),
    payload: JSON.stringify({ name: ad, parents: [klasor_().getId()], mimeType: 'application/zip' }),
    muteHttpExceptions: true,
  });
  if (yanit.getResponseCode() !== 200) {
    throw new Error('Drive ' + yanit.getResponseCode() + ': ' + yanit.getContentText().slice(0, 200));
  }
  const basliklar = yanit.getHeaders();
  const adres = basliklar['Location'] || basliklar['location'];
  if (!adres) throw new Error('Drive yükleme adresi vermedi');
  return 'YUKLE ' + adres;
}

function copu_buda_(klasor) {
  const sorgu = "'" + klasor.getId() + "' in parents and trashed = true";
  const yanit = UrlFetchApp.fetch(
    DRIVE + '?orderBy=createdTime%20desc&pageSize=200&fields=files(id,name)&q=' + encodeURIComponent(sorgu),
    { headers: yetki_(), muteHttpExceptions: true }
  );
  if (yanit.getResponseCode() !== 200) return 0;
  const fazla = JSON.parse(yanit.getContentText())
    .files.filter(function (dosya) { return dosya.name.indexOf(PREFIX) === 0; })
    .slice(COPTE_KALAN);
  let silinen = 0;
  fazla.forEach(function (dosya) {
    const sonuc = UrlFetchApp.fetch(DRIVE + '/' + dosya.id, {
      method: 'delete',
      headers: yetki_(),
      muteHttpExceptions: true,
    });
    if (sonuc.getResponseCode() === 204) silinen++;
  });
  return silinen;
}

function bitir_(sha, boyut) {
  const kisa = commit_(sha);
  const bayt = boyut_(boyut);
  const klasor = klasor_();
  const hepsi = yedekler_(klasor);
  const yeni = hepsi
    .filter(function (dosya) {
      return dosya.getName().indexOf(PREFIX + kisa + '_') === 0 && dosya.getSize() === bayt;
    })
    .sort(function (a, b) { return b.getDateCreated().getTime() - a.getDateCreated().getTime(); })[0];
  if (!yeni) throw new Error('yüklenen yedek klasörde yok ya da boyutu ' + bayt + ' bayt değil');
  let tasinan = 0;
  hepsi.forEach(function (dosya) {
    if (dosya.getId() !== yeni.getId()) {
      dosya.setTrashed(true);
      tasinan++;
    }
  });
  const silinen = copu_buda_(klasor);
  return 'yüklendi: ' + yeni.getName() + ' (' + yeni.getSize() + ' bayt), çöpe taşınan eski yedek: ' +
    tasinan + ', çöpten kalıcı silinen: ' + silinen;
}

function doGet(e) {
  if (!e || !e.parameter || e.parameter.key !== ozellik_('HOOK_KEY')) {
    return ContentService.createTextOutput('YETKISIZ');
  }
  const kilit = LockService.getScriptLock();
  if (!kilit.tryLock(30000)) return ContentService.createTextOutput('HATA meşgul (başka bir yedekleme sürüyor)');
  try {
    const islem = e.parameter.islem || 'durum';
    let sonuc;
    if (islem === 'durum') sonuc = durum_(e.parameter.sha);
    else if (islem === 'baslat') sonuc = baslat_(e.parameter.sha, e.parameter.boyut);
    else if (islem === 'bitir') sonuc = bitir_(e.parameter.sha, e.parameter.boyut);
    else throw new Error('bilinmeyen işlem: ' + islem);
    return ContentService.createTextOutput('OK ' + sonuc);
  } catch (hata) {
    return ContentService.createTextOutput('HATA ' + hata.message);
  } finally {
    kilit.releaseLock();
  }
}

function kur() {
  ozellik_('HOOK_KEY');
  const klasor = klasor_();
  const yanit = UrlFetchApp.fetch(DRIVE + '/' + klasor.getId() + '?fields=id,name', {
    headers: yetki_(),
    muteHttpExceptions: true,
  });
  if (yanit.getResponseCode() !== 200) {
    throw new Error('Drive API ' + yanit.getResponseCode() + ': Hizmetler → Drive API eklenmiş mi? ' +
      yanit.getContentText().slice(0, 200));
  }
  console.log('Hazır: "' + klasor.getName() + '" klasörü, yedek sayısı ' + yedekler_(klasor).length);
}
