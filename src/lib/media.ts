import { isWindows } from "./platform";
import type { Post } from "./posts";

const STATIC_ORIGIN = "https://static1.e621.net/";

/**
 * The webview cannot load static1 directly (hotlink protection rejects its cross-site requests),
 * so media goes through the app's `media` scheme. WebView2 exposes custom schemes as
 * `http://<scheme>.localhost`, WKWebView as `<scheme>://localhost`.
 */
export function mediaUrl(url: string): string {
  if (!url.startsWith(STATIC_ORIGIN)) throw new Error(`not an e621 media URL: ${url}`);
  const path = url.slice(STATIC_ORIGIN.length);
  return isWindows ? `http://media.localhost/${path}` : `media://localhost/${path}`;
}

/** What the viewer shows for a post, in the order report.md §5 sets. */
export type ViewerMedia =
  /** `display` is shown first; `original` once zooming needs more than its `displayWidth`. */
  | { kind: "image"; display: string; displayWidth: number; original: string; width: number; height: number }
  | { kind: "video"; url: string; width: number; height: number }
  | { kind: "missing"; deleted: boolean }
  | { kind: "unsupported"; ext: string };

const IMAGE_EXTENSIONS = new Set(["jpg", "jpeg", "png", "webp", "gif"]);

export function viewerMedia(post: Post): ViewerMedia {
  const { file } = post;
  if (file.url === null) return { kind: "missing", deleted: post.deleted };
  const video = post.video;
  if (video !== null) {
    const source = video.p720 ?? video.p480 ?? video.mp4 ?? video.original;
    if (source !== null) return { kind: "video", url: source.url, width: source.width, height: source.height };
  }
  if (file.ext === "webm" || file.ext === "mp4") {
    return { kind: "video", url: file.url, width: file.width, height: file.height };
  }
  if (!IMAGE_EXTENSIONS.has(file.ext)) return { kind: "unsupported", ext: file.ext };
  // A GIF's sample is a still frame, so the original is shown from the start.
  const sample = file.ext === "gif" ? null : post.sample;
  const display = sample?.url ?? file.url;
  const displayWidth = sample?.url ? sample.width : file.width;
  return { kind: "image", display, displayWidth, original: file.url, width: file.width, height: file.height };
}
