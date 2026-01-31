import { useState, useEffect } from 'react';
import { useSearchParams } from 'react-router-dom';
import { SchemaDiagram } from '../components/ui/SchemaDiagram';
import { Maximize2, Minimize2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { DatabaseProvider } from '../contexts/DatabaseProvider';
import { EditorProvider } from '../contexts/EditorProvider';

export const SchemaDiagramPage = () => {
  const { t } = useTranslation();
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [searchParams] = useSearchParams();
  const connectionId = searchParams.get('connectionId');

  const toggleFullscreen = () => {
    if (!document.fullscreenElement) {
      document.documentElement.requestFullscreen();
      setIsFullscreen(true);
    } else {
      document.exitFullscreen();
      setIsFullscreen(false);
    }
  };

  // Listen for fullscreen changes (e.g., ESC key)
  useEffect(() => {
    const handleFullscreenChange = () => {
      setIsFullscreen(!!document.fullscreenElement);
    };

    document.addEventListener('fullscreenchange', handleFullscreenChange);
    return () => {
      document.removeEventListener('fullscreenchange', handleFullscreenChange);
    };
  }, []);

  // Show error if no connectionId
  if (!connectionId) {
    return (
      <div className="w-screen h-screen flex items-center justify-center bg-slate-950">
        <div className="text-center">
          <h1 className="text-xl font-semibold text-slate-200 mb-2">
            {t('erDiagram.noConnection')}
          </h1>
          <p className="text-slate-400">
            {t('erDiagram.noConnectionDesc')}
          </p>
        </div>
      </div>
    );
  }

  return (
    <DatabaseProvider>
      <EditorProvider>
        <div className="w-screen h-screen flex flex-col bg-slate-950">
          {/* Minimal Header */}
          <div className="h-12 bg-slate-900 border-b border-slate-800 flex items-center justify-between px-4 shrink-0">
            <h1 className="text-slate-200 font-semibold">{t('erDiagram.title')}</h1>
            <button
              onClick={toggleFullscreen}
              className="flex items-center gap-2 px-3 py-1.5 bg-slate-800 hover:bg-slate-700 text-slate-200 rounded-lg border border-slate-700 transition-colors text-sm"
              title={isFullscreen ? t('erDiagram.exitFullscreen') : t('erDiagram.enterFullscreen')}
            >
              {isFullscreen ? (
                <>
                  <Minimize2 size={16} />
                  {t('erDiagram.exitFullscreen')}
                </>
              ) : (
                <>
                  <Maximize2 size={16} />
                  {t('erDiagram.enterFullscreen')}
                </>
              )}
            </button>
          </div>

          {/* Diagram Canvas */}
          <div className="flex-1 overflow-hidden">
            <SchemaDiagram connectionId={connectionId} />
          </div>
        </div>
      </EditorProvider>
    </DatabaseProvider>
  );
};
