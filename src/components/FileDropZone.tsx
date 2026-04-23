import { open } from "@tauri-apps/plugin-dialog";

interface FileDropZoneProps {
  onFileSelect: (path: string) => void;
  selectedFile: string | null;
}

const VIDEO_EXTENSIONS = [
  "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "ts", "m4v", "3gp",
];

export function FileDropZone({ onFileSelect, selectedFile }: FileDropZoneProps) {
  const handleClick = async () => {
    const file = await open({
      multiple: false,
      filters: [{ name: "Video Files", extensions: VIDEO_EXTENSIONS }],
    });
    if (file) {
      onFileSelect(file as string);
    }
  };

  const fileName = selectedFile
    ? selectedFile.split("/").pop() || selectedFile.split("\\").pop()
    : null;

  return (
    <div
      onClick={handleClick}
      className={`
        border-2 border-dashed rounded-xl p-10 text-center cursor-pointer
        transition-all duration-200
        ${
          selectedFile
            ? "border-green-500 bg-green-50 dark:bg-green-950/30"
            : "border-gray-300 dark:border-gray-600 hover:border-blue-400 hover:bg-blue-50 dark:hover:bg-blue-950/20"
        }
      `}
    >
      {selectedFile ? (
        <>
          <div className="text-3xl mb-3">✅</div>
          <p className="text-base font-medium text-green-700 dark:text-green-400">
            {fileName}
          </p>
          <p className="text-xs text-gray-500 mt-2">Click to change file</p>
        </>
      ) : (
        <>
          <div className="text-4xl mb-3">🎬</div>
          <p className="text-lg font-medium">Drop video file here</p>
          <p className="text-sm text-gray-500 dark:text-gray-400 mt-2">
            Or click to browse · MP4, MKV, AVI, MOV and more
          </p>
        </>
      )}
    </div>
  );
}
