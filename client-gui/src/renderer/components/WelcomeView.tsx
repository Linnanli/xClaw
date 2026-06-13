import { useState, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';
import { useIPC } from '../hooks/useIPC';
import { getInitialSessionTitle } from '../../shared/session-title';
import type { Message } from '../types';
import {
  buildContentBlocksFromComposerData,
  buildFileAttachmentsFromPaths,
  buildFileAttachmentsFromFiles,
  buildImageDraftsFromClipboardItems,
  buildImageDraftsFromFiles,
  type ComposerFileAttachment,
  type ComposerImage,
} from './composer/composerAdapters';
import { AssistantComposer } from './assistant-ui/composer/AssistantComposer';
import { AssistantRuntimeAdapter } from './assistant-ui/runtime/AssistantRuntimeAdapter';
import {
  AssistantSuggestionList,
  type AssistantSuggestionItem,
} from './assistant-ui/suggestion/AssistantSuggestionList';
import {
  FileText,
  BarChart3,
  FolderOpen,
  ArrowRight,
  Mail,
  Paperclip,
  BookOpen,
  FileSearch,
} from 'lucide-react';

import welcomeLogoSrc from '../assets/logo.png';

const welcomeRuntimeMessages: Message[] = [];

export function WelcomeView() {
  const { t } = useTranslation();
  const [prompt, setPrompt] = useState('');
  const [selectedTag, setSelectedTag] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [pastedImages, setPastedImages] = useState<ComposerImage[]>([]);
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

  const handleSubmit = async () => {
    if ((!prompt.trim() && pastedImages.length === 0 && attachedFiles.length === 0) || isSubmitting)
      return;

    const contentBlocks = buildContentBlocksFromComposerData({
      prompt,
      pastedImages,
      attachedFiles,
    });

    // Use the global working directory (always available after app startup)
    setIsSubmitting(true);
    try {
      const sessionTitle = getInitialSessionTitle(prompt, attachedFiles[0]?.name);
      const session = await startSession(sessionTitle, contentBlocks, workingDir || undefined);
      if (session) {
        setPrompt('');
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
    }
  };

  const quickTags: AssistantSuggestionItem[] = [
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
      badge: t('welcome.chromeRequired'),
    },
    {
      id: 'papers',
      label: t('welcome.searchPapers'),
      icon: BookOpen,
      prompt: t('welcome.quickPromptPapers'),
      badge: t('welcome.chromeRequired'),
    },
    {
      id: 'research-notion',
      label: t('welcome.summarizePapersToNotion'),
      icon: FileSearch,
      prompt: t('welcome.quickPromptNotion'),
      badge: t('welcome.notionRequired'),
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

        <AssistantRuntimeAdapter
          messages={welcomeRuntimeMessages}
          isRunning={isSubmitting}
          isSendDisabled={!canSubmit || isSubmitting}
          onNew={handleSubmit}
          onCancel={() => undefined}
        >
          <AssistantSuggestionList
            suggestions={quickTags}
            selectedId={selectedTag}
            onSelect={handleTagClick}
          />

          <AssistantComposer
            prompt={prompt}
            onPromptChange={setPrompt}
            textareaRef={textareaRef}
            isSubmitting={isSubmitting}
            pastedImages={pastedImages}
            attachedFiles={attachedFiles}
            onRemoveImage={removeImage}
            onRemoveFile={removeFile}
            onPaste={handlePaste}
            onSubmit={handleSubmit}
            placeholder={t('welcome.placeholder')}
            attachmentImageAlt={(index) => t('welcome.pastedImageAlt', { index: index + 1 })}
            onDragOver={handleDragOver}
            onDragLeave={handleDragLeave}
            onDrop={handleDrop}
            composerClassName={`rounded-[1.9rem] border border-border-muted bg-background/85 shadow-soft px-5 py-5 space-y-4 transition-colors ${
              isDragging ? 'ring-2 ring-accent bg-accent/5' : ''
            }`}
            layout="stacked"
            textareaClassName="w-full resize-none bg-transparent border-none outline-none text-text-primary placeholder:text-text-muted text-base leading-relaxed overflow-hidden"
            textareaStyle={{ minHeight: '72px', maxHeight: '200px' }}
            leftActions={
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
                    {workingDir
                      ? workingDir.split(/[/\\]/).pop()
                      : t('welcome.selectWorkingFolder')}
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
            }
            rightActions={
              <button
                type="submit"
                disabled={!canSubmit || isSubmitting}
                className="btn btn-primary px-5 py-2.5 rounded-2xl disabled:opacity-50 disabled:cursor-not-allowed"
              >
                <span>{isSubmitting ? t('welcome.starting') : t('welcome.letsGo')}</span>
                <ArrowRight className="w-4 h-4" />
              </button>
            }
          />
        </AssistantRuntimeAdapter>
      </div>
    </div>
  );
}
