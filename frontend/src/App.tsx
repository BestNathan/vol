import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { ConfigProvider } from 'antd';
import { AppLayout } from './components/AppLayout';
import { Dashboard } from './pages/Dashboard';
import { AgentTypes } from './pages/AgentTypes';
import { Instances } from './pages/Instances';
import { Chat } from './pages/Chat';

function App() {
  return (
    <ConfigProvider>
      <BrowserRouter>
        <AppLayout>
          <Routes>
            <Route path="/" element={<Dashboard />} />
            <Route path="/agent-types" element={<AgentTypes />} />
            <Route path="/instances" element={<Instances />} />
            <Route path="/chat/:agentType/:sessionId" element={<Chat />} />
          </Routes>
        </AppLayout>
      </BrowserRouter>
    </ConfigProvider>
  );
}

export default App;
