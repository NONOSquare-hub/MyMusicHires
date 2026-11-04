# KẾ HOẠCH CHI TIẾT DỰ ÁN: CLOUD HI-RES AUDIO PLAYER
**Nền tảng:** Android & Windows  
**Định hướng:** Mã nguồn mở, Tối ưu bộ nhớ (Zero-Disk Storage), Tự động hóa kết nối Google Drive, Chuẩn âm thanh Hi-Res (Bit-Perfect & An toàn Bluetooth).

---

## I. TỔNG QUAN DỰ ÁN

### 1. Mục tiêu sản phẩm
Xây dựng ứng dụng nghe nhạc cá nhân đa nền tảng (Android và Windows) với **2 nguồn phát âm thanh song song**:
* **Nguồn 1: Phát nhạc từ File cục bộ (Local Storage - Tính năng mặc định cốt lõi):**
  * Quét và phát trực tiếp file nhạc có sẵn trong máy (Bộ nhớ trong, Thẻ nhớ SD trên Android, Ổ đĩa cứng HDD/SSD trên Windows).
  * Hoạt động offline 100%, không cần kết nối mạng.
  * Tự động theo dõi thay đổi thư mục (Folder Watcher): Thêm/xóa file nhạc trên máy là app tự cập nhật ngay.
* **Nguồn 2: Stream nhạc từ Google Drive (Đám mây cá nhân):**
  * Nghe nhạc trực tuyến từ Google Drive không tốn dung lượng máy (Zero-Disk Storage qua bộ đệm RAM xoay vòng 20MB).
  * Đăng nhập 1 chạm tự động (OAuth2 PKCE với scope `drive.file` + Google Picker).
* **Thư viện hợp nhất (Unified Library - Chuẩn Roon):**
  * Gộp toàn bộ bài hát từ Bộ nhớ máy (Local) và Google Drive (Cloud) vào cùng một giao diện duy nhất.
  * Nhãn nhận diện trực quan: `[Local 💾]` (Nhạc trên máy) và `[Cloud ☁️]` (Nhạc trên Drive).
* **Hỗ trợ định dạng:** MP3, AAC, FLAC (16-bit / 24-bit Hi-Res).
* **Xử lý âm thanh thông minh:** Hỗ trợ WASAPI Exclusive trên Windows, chuẩn Direct Audio trên Android, và tự động tối ưu hóa khi chuyển đổi sang tai nghe Bluetooth.

---

## II. KIẾN TRÚC KỸ THUẬT & TECH STACK

```
┌─────────────────────────────────────────────────────────────┐
│                 GIAO DIỆN NGƯỜI DÙNG (UI/UX)                 │
│         Flutter (Dart) - Hỗ trợ chuẩn Android & Windows     │
└───────────────┬─────────────────────────────┬───────────────┘
                │                             │
                ▼                             ▼
┌───────────────────────────────┐ ┌───────────────────────────┐
│     MODULE GOOGLE DRIVE       │ │   QUẢN LÝ DỮ LIỆU CỤC BỘ   │
│ - OAuth2 PKCE (Google Sign-In)│ │ - SQLite (drift/sqflite)  │
│ - Scope: drive.file           │ │ - Secure Token Storage    │
│ - Google Picker UI            │ │ - WebP Thumbnail Cache    │
└───────────────┬───────────────┘ └───────────────────────────┘
                │
                ▼
┌─────────────────────────────────────────────────────────────┐
│             CORE AUDIO ENGINE: media_kit (libmpv)           │
│  - Hỗ trợ HTTP Range Streaming trực tiếp                    │
│  - Bộ đệm RAM xoay vòng (Ring Buffer 20MB, No Disk Cache)   │
└───────────────┬─────────────────────────────┬───────────────┘
                │                             │
                ▼ (Windows)                   ▼ (Android)
┌───────────────────────────────┐ ┌───────────────────────────┐
│     WINDOWS AUDIO OUTPUT      │ │    ANDROID AUDIO OUTPUT   │
│ - WASAPI Exclusive Mode       │ │ - AAudio / Direct PCM     │
│ - Tích hợp bàn phím SMTC      │ │ - Android 14 Bit-Perfect  │
│ - Fallback khi nghe Bluetooth │ │ - Foreground Service + UI │
└───────────────────────────────┘ └───────────────────────────┘
```

### Chi tiết lựa chọn công nghệ:
1. **Frontend / App Framework:** **Flutter** (Dart)
   * Tạo giao diện đồng nhất, mượt mà (60-120fps) trên cả Windows và Android từ một codebase duy nhất.
2. **Audio Engine Core:** **`media_kit`** (dựa trên nền tảng **`libmpv`** viết bằng C/C++)
   * Hỗ trợ giải mã FLAC 16/24-bit cực nhẹ.
   * Cho phép cấu hình bộ đệm trực tiếp trên RAM, không ghi file tạm ra ổ đĩa.
   * Tích hợp sẵn WASAPI Exclusive cho Windows.
3. **Database & Storage:**
   * **SQLite (`drift` hoặc `sqflite`):** Lưu trữ metadata bài hát (chỉ tốn ~2MB cho 1.000 bài).
   * **Flutter Secure Storage:** Lưu `refresh_token` an toàn trên Android Keystore và Windows Credential Manager.

---

## III. THIẾT KẾ CHI TIẾT CÁC MODULE CỐT LÕI

### Module 1: Quản lý & Phát nhạc từ File Cục bộ (Local Storage - Nền tảng mặc định)
* **Cơ chế chọn & Quản lý thư mục nguồn (Folder Manager - Học từ foobar2000 & Poweramp):**
  * **Tự chọn thư mục (Manual Folder Picker - Trọng tâm):**
    * Cho phép người dùng chủ động chọn đích danh các thư mục muốn đưa vào thư viện (ví dụ: `D:\Nhac_Lossless`, `E:\Audiophile_DSD`, hoặc thư mục cụ thể trên thẻ nhớ SD).
    * **Android:** Sử dụng **Storage Access Framework (SAF - `ACTION_OPEN_DOCUMENT_TREE`)** cho phép truy cập vĩnh viễn vào các thư mục đã chọn mà không cần xin quyền đọc toàn bộ bộ nhớ điện thoại (tránh vi phạm chính sách Google Play).
    * **Windows:** Tích hợp hộp thoại chọn thư mục chuẩn (`FolderPicker`).
  * **Danh sách thư mục theo dõi (Monitored Folders List):**
    * Người dùng có thể: Thêm thư mục mới, Tạm dừng quét (Toggle Bật/Tắt), Xóa thư mục khỏi thư viện, hoặc bấm "Quét lại ngay" (Rescan).
  * **Bộ lọc chống rác âm thanh (Anti-Junk Filter):**
    * Tự động bỏ qua các file âm thanh ngắn dưới 30 giây (nhạc chuông, âm thông báo, voice chat Zalo/Telegram).
    * Tự động bỏ qua các thư mục chứa file `.nomedia`.
* **Trích xuất Tag trực tiếp cực nhanh:**
  * Dùng thư viện đọc metadata native (`taglib` hoặc `audiotags`) đọc nhanh ID3v2, Vorbis comment, FLAC cuesheet từ file trên máy mà không tốn tài nguyên mạng.
* **Cơ chế Thư viện hợp nhất (Unified Library):**
  * Trong cơ sở dữ liệu SQLite, mỗi bài hát có trường `storage_type: 'LOCAL' | 'GOOGLE_DRIVE'`.
  * Người dùng có thể tạo một Playlist kết hợp cả bài hát nằm trong máy và bài hát nằm trên Google Drive.

### Module 2: Tự động hóa Xác thực & Kết nối Google Drive
* **Giao thức:** Google OAuth 2.0 với chuẩn **PKCE** (Proof Key for Code Exchange) an toàn cho Native App.
* **Phân quyền tối thiểu (Scope):** `https://www.googleapis.com/auth/drive.file`.
  * *Lợi ích:* Là scope **Non-Sensitive**, Google cấp phép miễn phí, không yêu cầu kiểm định an ninh đắt đỏ (CASA).
* **Luồng trải nghiệm người dùng (UX):**
  1. Người dùng bấm **"Đăng nhập với Google"**.
  2. Màn hình Google Picker hiển thị để người dùng chọn thư mục chứa nhạc (ví dụ: `Music_Lossless`).
  3. App nhận quyền đọc các file trong thư mục đã chọn và lưu `refresh_token`.
* **Cơ chế dự phòng (Advanced):** Cung cấp thêm ô nhập *Client ID / Secret cá nhân* trong mục Cài đặt nâng cao dành cho người dùng thích tự quản lý.

---

### Module 3: Cơ chế Stream Không Tốn Bộ Nhớ (Zero-Disk Storage)
* **Bộ đệm xoay vòng trên RAM (Memory Ring Buffer):**
  * Tham số cấu hình cho engine: `--demuxer-max-bytes=20971520` (giới hạn cứng 20MB trên RAM).
  * Chỉ nạp trước 30–60 giây của bài hát. Dữ liệu đã phát qua sẽ tự giải phóng khỏi RAM.
  * Ổ đĩa thiết bị **tăng 0 MB** khi phát nhạc.
* **Cơ chế tua bài (Seek):**
  * Tận dụng HTTP Range Header từ Google Drive API (`bytes=start-end`) thông qua Token xác thực để nhảy cóc dữ liệu tức thì mà không cần tải lại từ đầu bài.

---

### Module 3: Tối ưu Hóa Metadata & Ảnh Bìa (Cover Art)
* **Đọc dữ liệu 1 lần duy nhất:**
  * Lần đầu kết nối: Quét danh sách file qua Drive API (chỉ lấy `id`, `name`, `size`, `md5Checksum`).
  * Chỉ khi bài hát được click phát lần đầu, app mới trích xuất tag ID3/FLAC comment và lưu vào SQLite.
* **Nén ảnh bìa Album:**
  * Ảnh gốc trong file FLAC (thường từ 5MB–10MB) được resize ngay về kích thước `300x300` pixel và nén sang chuẩn **WebP** (~30KB).
  * 1.000 bài hát chỉ tốn tối đa ~30MB bộ nhớ thumbnail.

---

### Module 4: Quản lý Ngõ ra Âm thanh (Audio Pipeline & Bluetooth)
* **Trên Windows:**
  * Cung cấp nút chuyển đổi: **WASAPI Exclusive (Bit-Perfect)** hoặc **Shared Mode**.
  * Bắt sự kiện phím media trên bàn phím thông qua **SMTC** (System Media Transport Controls).
* **Trên Android:**
  * Chạy ngầm bằng `ForegroundService` kết hợp thông báo `MediaNotification` để không bị hệ điều hành tắt bài.
  * Hỗ trợ API `AudioMixerAttributes` trên Android 14+ để xuất âm thanh trực tiếp đến USB DAC rời.
* **Xử lý đặc biệt cho Tai nghe Bluetooth:**
  * **Auto-fallback:** Khi phát hiện thiết bị ra là Bluetooth (`TYPE_BLUETOOTH_A2DP`), app tự động tắt chế độ Exclusive để tránh xung đột driver.
  * **Pause on Disconnect:** Bắt sự kiện `ACTION_AUDIO_BECOMING_NOISY` để **dừng nhạc ngay lập tức** khi tai nghe Bluetooth ngắt kết nối hoặc hết pin.
  * **Audio Chain Badge:** Hiển thị rõ nguồn nhạc (Ví dụ: `FLAC 24-bit/96kHz`) và trạng thái xuất âm (Ví dụ: `Bluetooth (AAC/LDAC)` hoặc `Direct DAC`).

---

## IV. LỘ TRÌNH PHÁT TRIỂN (ROADMAP)

### Giai đoạn 1: Khởi tạo Kiến trúc & Core Player (Tuần 1 - 2)
- [ ] Khởi tạo dự án Flutter đa nền tảng (Android & Windows).
- [ ] Nhúng thư viện `media_kit` và kiểm tra khả năng phát file FLAC 24-bit nội bộ.
- [ ] Cấu hình cờ giới hạn RAM buffer để đảm bảo không ghi file tạm ra đĩa.
- [ ] Thiết lập bảng cơ sở dữ liệu SQLite để lưu Metadata bài hát.

### Giai đoạn 2: Kết nối Google Drive & Streaming (Tuần 3 - 4)
- [ ] Cấu hình OAuth2 PKCE trên Google Cloud Console với scope `drive.file`.
- [ ] Tích hợp Google Drive Picker để chọn thư mục nhạc.
- [ ] Xây dựng cơ chế truyền trực tiếp Access Token vào HTTP Stream Request của engine phát nhạc.
- [ ] Kiểm thử tính năng tua bài (Seek) trên file FLAC nặng (>100MB).

### Giai đoạn 3: Tối ưu Hóa Ngõ ra Âm thanh & Nền tảng (Tuần 5 - 6)
- [ ] **Windows:** Kích hoạt WASAPI Exclusive và tích hợp SMTC.
- [ ] **Android:** Viết Foreground Service, Notification Player và xử lý sự kiện `BECOMING_NOISY` (Bluetooth).
- [ ] Thêm nhãn Audio Chain info hiển thị chi tiết thông số bài hát.

### Giai đoạn 4: Hoàn thiện Giao diện & Kiểm thử (Tuần 7 - 8)
- [ ] Thiết kế giao diện nghe nhạc hiện đại (Now Playing, Playlist, Search, Quản lý thư mục Drive).
- [ ] Thử nghiệm trên nhiều điều kiện mạng (Wi-Fi, 4G, mất kết nối đột ngột).
- [ ] Kiểm thử tải lượng RAM và đảm bảo dung lượng lưu trữ không bị phình to.
- [ ] Đóng gói bộ cài đặt: File `.apk` cho Android và `.msix` / `.zip` cho Windows.

---

## V. MA TRẬN RỦI RO & PHƯƠNG ÁN XỬ LÝ

| Rủi ro tiềm ẩn | Mức độ | Phương án phòng chống |
| :--- | :---: | :--- |
| **Bị Google chặn API Rate Limit** | Thấp | Nhờ cơ chế RAM Buffer và SQLite Cache, số request giảm 95%. Mỗi bài hát chỉ tốn 2–4 requests. |
| **Dung lượng máy bị đầy** | Rất thấp | Không ghi file nhạc ra đĩa; ảnh bìa được nén sang WebP 300x300 (~30KB/ảnh). |
| **Nhạc bị tắt khi khóa màn hình (Android)** | Trung bình | Đăng ký Foreground Service chuẩn kèm `WAKE_LOCK` và xin người dùng cấp quyền bỏ qua tối ưu hóa pin. |
| **Lỗi giật âm thanh khi dùng Bluetooth** | Thấp | Tự động tắt Exclusive mode khi phát hiện ngõ ra Bluetooth; tăng nhẹ bộ đệm an toàn. |
