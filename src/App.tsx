import { useEffect } from 'react';
import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { MainLayout } from './components/layout/MainLayout';
import { Connections } from './pages/Connections';
import { Editor } from './pages/Editor';
import { Settings } from './pages/Settings';
import { SchemaDiagramPage } from './pages/SchemaDiagramPage';

function App() {
  useEffect(() => {
    const isLocalhost = window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1';
    
    if (isLocalhost) return;

    const handleContextMenu = (e: MouseEvent) => {
      e.preventDefault();
    };

    document.addEventListener('contextmenu', handleContextMenu);

    return () => {
      document.removeEventListener('contextmenu', handleContextMenu);
    };
  }, []);

  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<MainLayout />}>
          <Route index element={<Connections />} />
          <Route path="editor" element={<Editor />} />
          <Route path="settings" element={<Settings />} />
        </Route>
        <Route path="/schema-diagram" element={<SchemaDiagramPage />} />
      </Routes>
    </BrowserRouter>
  );
}

export default App;
