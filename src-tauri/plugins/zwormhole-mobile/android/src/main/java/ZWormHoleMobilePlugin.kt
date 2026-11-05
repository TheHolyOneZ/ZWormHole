package com.zwormhole.mobile

import android.Manifest
import android.app.Activity
import android.app.DownloadManager
import android.content.ActivityNotFoundException
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.media.MediaScannerConnection
import android.net.Uri
import android.net.wifi.WifiManager
import android.os.Build
import android.os.Environment
import android.os.Handler
import android.os.Looper
import android.provider.DocumentsContract
import android.provider.OpenableColumns
import android.provider.Settings
import android.webkit.MimeTypeMap
import android.webkit.WebView
import androidx.activity.result.ActivityResult
import androidx.appcompat.app.AppCompatActivity
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import androidx.core.content.FileProvider
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.io.IOException

@InvokeArg
class PickArgs {
    var dir: String = ""
    var multiple: Boolean = true
    var images: Boolean = false
}

@InvokeArg
class DirArgs {
    var dir: String = ""
}

@InvokeArg
class ExportArgs {
    var source: String = ""
    var name: String = "export.txt"
    var mime: String = "text/plain"
}

@InvokeArg
class PathArgs {
    var path: String = ""
}

@InvokeArg
class PathsArgs {
    var paths: Array<String> = arrayOf()
}

@InvokeArg
class ServiceArgs {
    var enabled: Boolean = false
    var title: String = ""
    var text: String = ""
}

@TauriPlugin
class ZWormHoleMobilePlugin(private val activity: Activity) : Plugin(activity) {
    private var multicastLock: WifiManager.MulticastLock? = null
    private var pendingDir: String = ""
    private var pendingExportSource: String = ""
    private val main = Handler(Looper.getMainLooper())

    override fun load(webView: WebView) {
        super.load(webView)
        showLightSystemBars()
        try {
            val wifi = activity.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
            multicastLock = wifi.createMulticastLock("zwormhole-discovery").apply {
                setReferenceCounted(false)
                acquire()
            }
        } catch (_: Exception) {
        }
        if (Build.VERSION.SDK_INT <= Build.VERSION_CODES.Q &&
            ContextCompat.checkSelfPermission(activity, Manifest.permission.WRITE_EXTERNAL_STORAGE) != PackageManager.PERMISSION_GRANTED
        ) {
            ActivityCompat.requestPermissions(activity, arrayOf(Manifest.permission.WRITE_EXTERNAL_STORAGE), 4711)
        }
    }

    override fun onResume() {
        super.onResume()
        showLightSystemBars()
    }

    private fun showLightSystemBars() {
        activity.runOnUiThread {
            val window = activity.window
            val bars = WindowCompat.getInsetsController(window, window.decorView)
            bars.show(WindowInsetsCompat.Type.systemBars())
            bars.isAppearanceLightStatusBars = false
            bars.isAppearanceLightNavigationBars = false
        }
    }

    override fun onDestroy(activity: AppCompatActivity) {
        multicastLock?.let { if (it.isHeld) it.release() }
        multicastLock = null
        super.onDestroy(activity)
    }

    private fun background(invoke: Invoke, work: () -> JSObject) {
        Thread {
            try {
                val result = work()
                main.post { invoke.resolve(result) }
            } catch (e: Exception) {
                main.post { invoke.reject(e.message ?: e.javaClass.simpleName) }
            }
        }.start()
    }

    private fun cancelled(): JSObject {
        val o = JSObject()
        o.put("cancelled", true)
        return o
    }

    private fun sanitize(name: String?): String {
        val cleaned = (name ?: "").replace(Regex("[/\\\\\u0000]"), "_").trim()
        return if (cleaned.isEmpty() || cleaned == "." || cleaned == "..") "file" else cleaned
    }

    private fun uniqueFile(dir: File, name: String): File {
        var f = File(dir, name)
        if (!f.exists()) return f
        val dot = name.lastIndexOf('.')
        val base = if (dot > 0) name.substring(0, dot) else name
        val ext = if (dot > 0) name.substring(dot) else ""
        var i = 2
        while (f.exists()) {
            f = File(dir, "$base ($i)$ext")
            i++
        }
        return f
    }

    private fun displayName(uri: Uri): String {
        var name: String? = null
        try {
            activity.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
                if (c.moveToFirst()) {
                    val i = c.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                    if (i >= 0) name = c.getString(i)
                }
            }
        } catch (_: Exception) {
        }
        return sanitize(name ?: uri.lastPathSegment)
    }

    private fun copyUri(uri: Uri, target: File) {
        val input = activity.contentResolver.openInputStream(uri) ?: throw IOException("Cannot read $uri")
        input.use { src -> FileOutputStream(target).use { dst -> src.copyTo(dst, 1 shl 20) } }
    }

    private fun copyTree(treeUri: Uri, docId: String, target: File) {
        target.mkdirs()
        val children = DocumentsContract.buildChildDocumentsUriUsingTree(treeUri, docId)
        val projection = arrayOf(
            DocumentsContract.Document.COLUMN_DOCUMENT_ID,
            DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            DocumentsContract.Document.COLUMN_MIME_TYPE
        )
        activity.contentResolver.query(children, projection, null, null, null)?.use { c ->
            while (c.moveToNext()) {
                val id = c.getString(0)
                val name = sanitize(c.getString(1) ?: id)
                val mime = c.getString(2)
                if (mime == DocumentsContract.Document.MIME_TYPE_DIR) {
                    copyTree(treeUri, id, File(target, name))
                } else {
                    copyUri(DocumentsContract.buildDocumentUriUsingTree(treeUri, id), uniqueFile(target, name))
                }
            }
        }
    }

    private fun treeName(treeUri: Uri, docId: String): String {
        val docUri = DocumentsContract.buildDocumentUriUsingTree(treeUri, docId)
        var name: String? = null
        try {
            activity.contentResolver.query(
                docUri,
                arrayOf(DocumentsContract.Document.COLUMN_DISPLAY_NAME),
                null,
                null,
                null
            )?.use { c -> if (c.moveToFirst()) name = c.getString(0) }
        } catch (_: Exception) {
        }
        return sanitize(name ?: docId.substringAfterLast('/').substringAfterLast(':'))
    }

    @Command
    fun pickFiles(invoke: Invoke) {
        val args = invoke.parseArgs(PickArgs::class.java)
        pendingDir = args.dir
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            type = if (args.images) "image/*" else "*/*"
            putExtra(Intent.EXTRA_ALLOW_MULTIPLE, args.multiple)
        }
        startActivityForResult(invoke, intent, "pickFilesResult")
    }

    @ActivityCallback
    fun pickFilesResult(invoke: Invoke, result: ActivityResult) {
        val data = result.data
        if (result.resultCode != Activity.RESULT_OK || data == null) {
            invoke.resolve(cancelled())
            return
        }
        val uris = mutableListOf<Uri>()
        val clip = data.clipData
        if (clip != null) {
            for (i in 0 until clip.itemCount) uris.add(clip.getItemAt(i).uri)
        } else {
            data.data?.let { uris.add(it) }
        }
        val dir = File(pendingDir)
        background(invoke) {
            dir.mkdirs()
            val paths = JSArray()
            for (uri in uris) {
                val target = uniqueFile(dir, displayName(uri))
                copyUri(uri, target)
                paths.put(target.absolutePath)
            }
            val o = JSObject()
            o.put("paths", paths)
            o
        }
    }

    @Command
    fun pickFolder(invoke: Invoke) {
        val args = invoke.parseArgs(DirArgs::class.java)
        pendingDir = args.dir
        startActivityForResult(invoke, Intent(Intent.ACTION_OPEN_DOCUMENT_TREE), "pickFolderResult")
    }

    @ActivityCallback
    fun pickFolderResult(invoke: Invoke, result: ActivityResult) {
        val treeUri = result.data?.data
        if (result.resultCode != Activity.RESULT_OK || treeUri == null) {
            invoke.resolve(cancelled())
            return
        }
        val dir = File(pendingDir)
        background(invoke) {
            val rootId = DocumentsContract.getTreeDocumentId(treeUri)
            val target = uniqueFile(dir.also { it.mkdirs() }, treeName(treeUri, rootId))
            copyTree(treeUri, rootId, target)
            val o = JSObject()
            o.put("path", target.absolutePath)
            o
        }
    }

    @Command
    fun pickSaveDir(invoke: Invoke) {
        startActivityForResult(invoke, Intent(Intent.ACTION_OPEN_DOCUMENT_TREE), "pickSaveDirResult")
    }

    @ActivityCallback
    fun pickSaveDirResult(invoke: Invoke, result: ActivityResult) {
        val treeUri = result.data?.data
        if (result.resultCode != Activity.RESULT_OK || treeUri == null) {
            invoke.resolve(cancelled())
            return
        }
        val path = treePath(treeUri)
        if (path == null) {
            invoke.reject("That location can't be used as a save folder. Pick a folder on the internal storage or SD card.")
            return
        }
        val o = JSObject()
        o.put("path", path)
        invoke.resolve(o)
    }

    private fun treePath(treeUri: Uri): String? {
        val docId = DocumentsContract.getTreeDocumentId(treeUri)
        if (docId.startsWith("raw:")) return docId.removePrefix("raw:")
        if (treeUri.authority == "com.android.providers.downloads.documents") {
            return if (docId == "downloads") publicDownloads() else null
        }
        if (treeUri.authority != "com.android.externalstorage.documents") return null
        val volume = docId.substringBefore(':')
        val rel = docId.substringAfter(':', "")
        val base = when (volume) {
            "primary" -> Environment.getExternalStorageDirectory().absolutePath
            "home" -> File(Environment.getExternalStorageDirectory(), Environment.DIRECTORY_DOCUMENTS).absolutePath
            else -> "/storage/$volume"
        }
        return if (rel.isEmpty()) base else "$base/$rel"
    }

    private fun publicDownloads(): String =
        Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS).absolutePath

    @Command
    fun exportFile(invoke: Invoke) {
        val args = invoke.parseArgs(ExportArgs::class.java)
        pendingExportSource = args.source
        val intent = Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            type = args.mime
            putExtra(Intent.EXTRA_TITLE, args.name)
        }
        startActivityForResult(invoke, intent, "exportFileResult")
    }

    @ActivityCallback
    fun exportFileResult(invoke: Invoke, result: ActivityResult) {
        val uri = result.data?.data
        if (result.resultCode != Activity.RESULT_OK || uri == null) {
            invoke.resolve(cancelled())
            return
        }
        val source = File(pendingExportSource)
        background(invoke) {
            val out = activity.contentResolver.openOutputStream(uri, "wt") ?: throw IOException("Cannot write $uri")
            out.use { dst -> FileInputStream(source).use { src -> src.copyTo(dst) } }
            val o = JSObject()
            o.put("saved", true)
            o
        }
    }

    @Command
    fun clipboardImage(invoke: Invoke) {
        val args = invoke.parseArgs(DirArgs::class.java)
        val cm = activity.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val clip = cm.primaryClip
        val uris = mutableListOf<Uri>()
        if (clip != null) {
            for (i in 0 until clip.itemCount) clip.getItemAt(i).uri?.let { uris.add(it) }
        }
        background(invoke) {
            val o = JSObject()
            for (uri in uris) {
                val mime = activity.contentResolver.getType(uri) ?: ""
                if (!mime.startsWith("image/")) continue
                val bitmap = activity.contentResolver.openInputStream(uri)?.use { BitmapFactory.decodeStream(it) } ?: continue
                val dir = File(args.dir).also { it.mkdirs() }
                val target = uniqueFile(dir, "clipboard-${System.currentTimeMillis()}.png")
                FileOutputStream(target).use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
                o.put("path", target.absolutePath)
                break
            }
            o
        }
    }

    @Command
    fun deviceInfo(invoke: Invoke) {
        val cr = activity.contentResolver
        var name: String? = null
        try {
            name = Settings.Global.getString(cr, "device_name")
        } catch (_: Exception) {
        }
        if (name.isNullOrBlank()) {
            try {
                name = Settings.Secure.getString(cr, "bluetooth_name")
            } catch (_: Exception) {
            }
        }
        if (name.isNullOrBlank()) {
            val maker = Build.MANUFACTURER.replaceFirstChar { it.uppercase() }
            name = if (Build.MODEL.startsWith(maker, ignoreCase = true)) Build.MODEL else "$maker ${Build.MODEL}"
        }
        val o = JSObject()
        o.put("name", name)
        o.put("downloadDir", publicDownloads())
        invoke.resolve(o)
    }

    @Command
    fun openFile(invoke: Invoke) {
        val args = invoke.parseArgs(PathArgs::class.java)
        val file = File(args.path)
        if (file.isDirectory) {
            reveal(file)
            invoke.resolve()
            return
        }
        try {
            val uri = FileProvider.getUriForFile(activity, activity.packageName + ".zwormhole.files", file)
            val mime = MimeTypeMap.getSingleton().getMimeTypeFromExtension(file.extension.lowercase()) ?: "*/*"
            val view = Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(uri, mime)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            activity.startActivity(Intent.createChooser(view, file.name).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION))
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject(e.message ?: "No app can open this file")
        }
    }

    @Command
    fun revealFile(invoke: Invoke) {
        val args = invoke.parseArgs(PathArgs::class.java)
        val file = File(args.path)
        reveal(if (file.isDirectory) file else file.parentFile ?: file)
        invoke.resolve()
    }

    private fun reveal(dir: File) {
        val root = Environment.getExternalStorageDirectory().absolutePath
        val path = dir.absolutePath
        if (path.startsWith(root)) {
            val rel = path.removePrefix(root).trim('/')
            val docUri = DocumentsContract.buildDocumentUri("com.android.externalstorage.documents", "primary:$rel")
            val view = Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(docUri, DocumentsContract.Document.MIME_TYPE_DIR)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            try {
                activity.startActivity(view)
                return
            } catch (_: ActivityNotFoundException) {
            } catch (_: SecurityException) {
            }
        }
        try {
            activity.startActivity(Intent(DownloadManager.ACTION_VIEW_DOWNLOADS))
        } catch (_: Exception) {
        }
    }

    @Command
    fun mediaScan(invoke: Invoke) {
        val args = invoke.parseArgs(PathsArgs::class.java)
        val files = mutableListOf<String>()
        for (p in args.paths) {
            val f = File(p)
            if (f.isDirectory) {
                f.walkTopDown().filter { it.isFile }.forEach { files.add(it.absolutePath) }
            } else if (f.isFile) {
                files.add(f.absolutePath)
            }
        }
        if (files.isNotEmpty()) {
            MediaScannerConnection.scanFile(activity.applicationContext, files.toTypedArray(), null, null)
        }
        invoke.resolve()
    }

    @Command
    fun setBackgroundService(invoke: Invoke) {
        val args = invoke.parseArgs(ServiceArgs::class.java)
        val ctx = activity.applicationContext
        val intent = Intent(ctx, ListenerService::class.java)
        try {
            if (args.enabled) {
                intent.putExtra(ListenerService.EXTRA_TITLE, args.title)
                intent.putExtra(ListenerService.EXTRA_TEXT, args.text)
                ContextCompat.startForegroundService(ctx, intent)
            } else {
                ctx.stopService(intent)
            }
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject(e.message ?: "Could not change the background service")
        }
    }
}
