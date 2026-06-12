import { useState, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';
import { useIPC } from '../hooks/useIPC';
import { getInitialSessionTitle } from '../../shared/session-title';
import {
  buildContentBlocksFromComposerData,
  buildFileAttachmentsFromPaths,
  buildFileAttachmentsFromFiles,
  buildImageDraftsFromClipboardItems,
  buildImageDraftsFromFiles,
  type ComposerFileAttachment,
  type ComposerImage,
} from './composer/composerAdapters';
import {
  FileText,
  BarChart3,
  FolderOpen,
  ArrowRight,
  Mail,
  X,
  Paperclip,
  BookOpen,
  FileSearch,
} from 'lucide-react';

import welcomeLogoSrc from '../assets/logo.png';

export function WelcomeView() {
  const { t } = useTranslation();
  const [prompt, setPrompt] = useState('');
  const [selectedTag, setSelectedTag] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const isComposingRef = useRef(false);
  const [pastedImages, setPastedImages] = useState<
    ComposerImage[]
  >([]);
  const [attachedFiles, setAttachedFiles] = useState<ComposerFileAttachment[]>([]);
  const [isDragging, setIsDragging] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const { startSession, changeWorkingDir, isElectron } = useIPC();
  const workingDir = useAppStore((state) => state.workingDir);
  const setGlobalNotice = useAppStore((state) => state.setGlobalNotice);
  const isConfigured = useAppStore((state) => state.isConfigured);
  const setShowSettings = useAppStore((state) => state.setShowSettings);
  const setSettingsTab = useAppStore((state) => state.setSettingsTab);
  const canSubmit = prompt.trim().length > 0 || pastedImages.length > 0 || attachedFiles.length > 0;

  const handleSelectFolder = async () => {
    try {
      const result = await changeWorkingDir(undefined, workingDir || undefined);
      if (!result.success && result.error && result.error !== 'User cancelled') {
        setGlobalNotice({
          id: `notice-workdir-select-${Date.now()}`,
          type: 'warning',
          message: `${t('welcome.selectWorkingFolderFailed')}: ${result.error}`,
        });
      }
    } catch (error) {
      setGlobalNotice({
        id: `notice-workdir-select-${Date.now()}`,
        type: 'error',
        message:
          error instanceof Error && error.message
            ? `${t('welcome.selectWorkingFolderFailed')}: ${error.message}`
            : t('welcome.selectWorkingFolderFailed'),
      });
    }
  };

  // Handle paste event for images
  const handlePaste = async (e: React.ClipboardEvent) => {
    const items = e.clipboardData?.items;
    const imageItems = await buildImageDraftsFromClipboardItems(items);
    if (imageItems.length === 0) return;

    e.preventDefault();
    setPastedImages((prev) => [...prev, ...imageItems]);
  };

  const removeImage = (index: number) => {
    setPastedImages((prev) => {
      const updated = [...prev];
      URL.revokeObjectURL(updated[index].url);
      updated.splice(index, 1);
      return updated;
    });
  };

  const removeFile = (index: number) => {
    setAttachedFiles((prev) => {
      const updated = [...prev];
      updated.splice(index, 1);
      return updated;
    });
  };

  const handleFileSelect = async () => {
    if (!isElectron || !window.electronAPI) {
      console.log('[WelcomeView] Not in Electron, file selection not available');
      return;
    }

    try {
      const filePaths = await window.electronAPI.selectFiles();
      if (filePaths.length === 0) return;

      setAttachedFiles((prev) => [...prev, ...buildFileAttachmentsFromPaths(filePaths)]);
    } catch (error) {
      console.error('[WelcomeView] Error selecting files:', error);
    }
  };

  // Handle drag and drop for images
  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(true);
  };

  const handleDragLeave = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);
  };

  const handleDrop = async (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);

    const files = Array.from(e.dataTransfer.files);
    const newImages = await buildImageDraftsFromFiles(files);
    const newFiles = await buildFileAttachmentsFromFiles(files);

    if (newImages.length > 0) {
      setPastedImages((prev) => [...prev, ...newImages]);
    }

    if (newFiles.length > 0) {
      setAttachedFiles((prev) => [...prev, ...newFiles]);
    }
  };

  const handleSubmit = async (e?: React.FormEvent) => {
    e?.preventDefault();

    // Get value from ref to handle both controlled and uncontrolled cases
    const currentPrompt = textareaRef.current?.value || prompt;

    if (
      (!currentPrompt.trim() && pastedImages.length === 0 && attachedFiles.length === 0) ||
      isSubmitting
    )
      return;

    const contentBlocks = buildContentBlocksFromComposerData({
      prompt: currentPrompt,
      pastedImages,
      attachedFiles,
    });

    // Use the global working directory (always available after app startup)
    setIsSubmitting(true);
    try {
      const sessionTitle = getInitialSessionTitle(currentPrompt, attachedFiles[0]?.name);
      const session = await startSession(sessionTitle, contentBlocks, workingDir || undefined);
      if (session) {
        setPrompt('');
        if (textareaRef.current) {
          textareaRef.current.value = '';
        }
        pastedImages.forEach((img) => URL.revokeObjectURL(img.url));
        setPastedImages([]);
        setAttachedFiles([]);
      }
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleTagClick = (tag: string, tagPrompt: string) => {
    setSelectedTag(tag === selectedTag ? null : tag);
    if (tag !== selectedTag) {
      setPrompt(tagPrompt);
      if (textareaRef.current) {
        textareaRef.current.value = tagPrompt;
        // Trigger height adjustment
        adjustTextareaHeight();
      }
    }
  };

  // Auto-adjust textarea height based on content
  const adjustTextareaHeight = () => {
    const textarea = textareaRef.current;
    if (textarea) {
      // Reset height to auto to get the correct scrollHeight
      textarea.style.height = 'auto';
      // Set max height to 200px (about 8 lines), then scroll
      const maxHeight = 200;
      const newHeight = Math.min(textarea.scrollHeight, maxHeight);
      textarea.style.height = `${newHeight}px`;
      // Show scrollbar if content exceeds max height
      textarea.style.overflowY = textarea.scrollHeight > maxHeight ? 'auto' : 'hidden';
    }
  };

  // Adjust height when prompt changes
  useEffect(() => {
    adjustTextareaHeight();
  }, [prompt]);

  const quickTags = [
    {
      id: 'create',
      label: t('welcome.createFile'),
      icon: FileText,
      prompt: t('welcome.quickPromptCreate'),
    },
    {
      id: 'crunch',
      label: t('welcome.crunchData'),
      icon: BarChart3,
      prompt: t('welcome.quickPromptCrunch'),
    },
    {
      id: 'organize',
      label: t('welcome.organizeFiles'),
      icon: FolderOpen,
      prompt: t('welcome.quickPromptOrganize'),
    },
    {
      id: 'email',
      label: t('welcome.checkEmails'),
      icon: Mail,
      prompt: t('welcome.quickPromptEmail'),
      requiresChrome: true,
    },
    {
      id: 'papers',
      label: t('welcome.searchPapers'),
      icon: BookOpen,
      prompt: t('welcome.quickPromptPapers'),
      requiresChrome: true,
    },
    {
      id: 'research-notion',
      label: t('welcome.summarizePapersToNotion'),
      icon: FileSearch,
      prompt: t('welcome.quickPromptNotion'),
      requiresNotion: true,
    },
  ];

  return (
    <div className="flex-1 flex flex-col items-center justify-center px-5 py-10 md:px-8 md:py-14">
      <div className="max-w-[840px] w-full space-y-7 animate-fade-in">
        <div className="space-y-4 text-center">
          <div className="flex items-center justify-center gap-4">
            <img
              src={welcomeLogoSrc}
              alt={t('welcome.logoAlt')}
              className="w-16 h-16 md:w-20 md:h-20 rounded-[1.4rem] object-cover border border-border-subtle bg-background/60 shadow-soft"
            />
            <div className="text-left">
              <h1 className="text-[2.35rem] md:text-[3.1rem] leading-none font-semibold tracking-[-0.05em] text-text-primary">
                Open Cowork
              </h1>
            </div>
          </div>
          <p className="heading-serif text-[1.15rem] md:text-[1.45rem] font-medium tracking-[-0.02em] text-text-secondary text-center">
            {t('welcome.title')}
          </p>
        </div>

        {/* API Not Configured Hint */}
        {!isConfigured && (
          <p className="text-sm text-text-muted text-center">
            {t('welcome.apiNotConfigured')}{' '}
            <button
              type="button"
              onClick={() => {
                setSettingsTab('api');
                setShowSettings(true);
              }}
              className="inline-flex items-center gap-1 text-accent hover:text-accent-hover transition-colors"
            >
              {t('welcome.goToSettings')}
              <ArrowRight className="w-3.5 h-3.5" />
            </button>
          </p>
        )}

        {/* Quick Action Tags */}
        <div className="flex flex-wrap gap-2 justify-center px-3">
          {quickTags.map((tag) => (
            <button
              key={tag.id}
              onClick={() => handleTagClick(tag.id, tag.prompt)}
              className={`inline-flex items-center gap-2 rounded-full border px-3 py-2 text-sm transition-colors ${
                selectedTag === tag.id
                  ? 'border-accent/30 bg-accent-muted text-accent'
                  : 'border-border-subtle bg-background/65 text-text-secondary hover:bg-surface-hover hover:text-text-primary'
              } ${
                ('requiresChrome' in tag && tag.requiresChrome) ||
                ('requiresNotion' in tag && tag.requiresNotion)
                  ? 'relative'
                  : ''
              }`}
            >
              <tag.icon
                className={`w-4 h-4 ${selectedTag === tag.id ? 'text-accent' : 'text-text-muted'}`}
              />
              <span>{tag.label}</span>
              {'requiresChrome' in tag && tag.requiresChrome && (
                <span className="ml-1 px-1.5 py-px text-[9px] rounded bg-surface-active text-text-muted">
                  {t('welcome.chromeRequired')}
                </span>
              )}
              {'requiresNotion' in tag && tag.requiresNotion && (
                <span className="ml-1 px-1.5 py-px text-[9px] rounded bg-surface-active text-text-muted">
                  {t('welcome.notionRequired')}
                </span>
              )}
            </button>
          ))}
        </div>

        {/* Main Input Card - Right aligned */}
        <form
          onSubmit={handleSubmit}
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
          className={`rounded-[1.9rem] border border-border-muted bg-background/85 shadow-soft px-5 py-5 space-y-4 transition-colors ${
            isDragging ? 'ring-2 ring-accent bg-accent/5' : ''
          }`}
        >
          {/* Image previews */}
          {pastedImages.length > 0 && (
            <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-2 pb-2 border-b border-border w-full">
              {pastedImages.map((img, index) => (
                <div key={index} className="relative group">
                  <img
                    src={img.url}
                    alt={t('welcome.pastedImageAlt', { index: index + 1 })}
                    className="w-full aspect-square object-cover rounded-lg border border-border"
                  />
                  <button
                    type="button"
                    onClick={() => removeImage(index)}
                    className="absolute -top-1 -right-1 w-5 h-5 rounded-full bg-error text-white flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity"
                  >
                    <X className="w-3 h-3" />
                  </button>
                </div>
              ))}
            </div>
          )}

          {/* File attachments */}
          {attachedFiles.length > 0 && (
            <div className="space-y-2 mb-3">
              {attachedFiles.map((file, index) => (
                <div
                  key={index}
                  className="flex items-center gap-2 px-3 py-2 rounded-lg bg-surface-muted border border-border group"
                >
                  <div className="flex-1 min-w-0">
                    <p className="text-sm text-text-primary truncate">{file.name}</p>
                  </div>
                  <button
                    type="button"
                    onClick={() => removeFile(index)}
                    className="w-6 h-6 rounded-full bg-error/10 hover:bg-error/20 text-error flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity"
                  >
                    <X className="w-3.5 h-3.5" />
                  </button>
                </div>
              ))}
            </div>
          )}

          {/* Text Input - Auto-resizing */}
          <textarea
            ref={textareaRef}
            value={prompt}
            onChange={(e) => {
              setPrompt(e.target.value);
              adjustTextareaHeight();
            }}
            onCompositionStart={() => {
              isComposingRef.current = true;
            }}
            onCompositionEnd={() => {
              isComposingRef.current = false;
            }}
            onPaste={handlePaste}
            placeholder={t('welcome.placeholder')}
            rows={1}
            style={{ minHeight: '72px', maxHeight: '200px' }}
            className="w-full resize-none bg-transparent border-none outline-none text-text-primary placeholder:text-text-muted text-base leading-relaxed overflow-hidden"
            onKeyDown={(e) => {
              // Enter to send, Shift+Enter for new line
              if (e.key === 'Enter' && !e.shiftKey) {
                if (e.nativeEvent.isComposing || isComposingRef.current || e.keyCode === 229) {
                  return;
                }
                e.preventDefault();
                handleSubmit();
              }
            }}
          />

          {/* Bottom Actions */}
          <div className="flex items-center justify-between pt-3 border-t border-border-muted">
            <div className="flex items-center gap-3">
              <button
                type="button"
                onClick={handleSelectFolder}
                className={`flex items-center gap-2 text-sm transition-colors ${
                  workingDir
                    ? 'text-text-secondary hover:text-text-primary'
                    : 'text-accent hover:text-accent-hover'
                }`}
                title={workingDir || t('welcome.selectWorkingFolder')}
              >
                <FolderOpen className="w-4 h-4" />
                <span>
                  {workingDir ? workingDir.split(/[/\\]/).pop() : t('welcome.selectWorkingFolder')}
                </span>
              </button>

              {isElectron && (
                <button
                  type="button"
                  onClick={handleFileSelect}
                  className="flex items-center gap-2 text-sm text-text-secondary hover:text-text-primary transition-colors"
                >
                  <Paperclip className="w-4 h-4" />
                  <span>{t('welcome.attachFiles')}</span>
                </button>
              )}
            </div>

            <button
              type="submit"
              disabled={!canSubmit || isSubmitting}
              className="btn btn-primary px-5 py-2.5 rounded-2xl disabled:opacity-50 disabled:cursor-not-allowed"
            >
              <span>{isSubmitting ? t('welcome.starting') : t('welcome.letsGo')}</span>
              <ArrowRight className="w-4 h-4" />
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
