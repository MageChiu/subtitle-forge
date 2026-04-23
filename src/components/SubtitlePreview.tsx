interface SubtitlePreviewProps {
  filePath: string;
}

export function SubtitlePreview({ filePath }: SubtitlePreviewProps) {
  // In a real implementation, this would read the file via Tauri FS API
  // and display the subtitle entries in a scrollable list

  return (
    <div className="bg-white dark:bg-gray-800 rounded-xl p-5 border border-gray-200 dark:border-gray-700">
      <h3 className="font-medium mb-3">Subtitle Preview</h3>
      <div className="bg-gray-900 rounded-lg p-4 text-center min-h-[120px] flex items-center justify-center">
        <div>
          <p className="text-white text-lg">Hello, world!</p>
          <p className="text-yellow-300 text-sm mt-1">你好，世界！</p>
        </div>
      </div>
      <p className="text-xs text-gray-500 mt-3 font-mono break-all">
        {filePath}
      </p>
    </div>
  );
}
