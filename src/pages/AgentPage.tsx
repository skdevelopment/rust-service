/**
 * Agent Page
 *
 * The chat view for ServiceAgent. All agent state lives in the
 * AgentSessionProvider (above the tabs), so leaving this tab never interrupts
 * a running turn or loses pending approvals.
 */

import { useCallback, useMemo, useState } from 'react';
import { useAgentSession } from '@/components/agent-session-context';
import { toolSummary as buildToolSummary } from '@/lib/agent-tools';
import { navigateTo } from '@/lib/navigation';
import { AgentRightSidebar } from '@/components/agent/AgentRightSidebar';
import { ConversationSelector } from '@/components/agent/ConversationSelector';
import { AgentMessageList } from '@/components/agent/AgentMessageList';
import { AgentInputArea } from '@/components/agent/AgentInputArea';
import type { ApprovalMode } from '@/types/agent';
import {
  Bot,
  Sparkles,
  Shield,
  ShieldAlert,
  ShieldOff,
  Square,
  Zap,
  Menu,
  PanelRightClose,
  PanelRight,
  AlertTriangle,
  RotateCcw,
} from 'lucide-react';
import { cn } from '@/lib/utils';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { useSettings } from '@/components/settings-context';
import { useAnimation, motion, AnimatePresence } from '@/components/animation-context';

// =============================================================================
// Small Components
// =============================================================================

function ApprovalModeIndicator({ mode }: { mode: ApprovalMode }) {
  const config = {
    always: {
      icon: Shield,
      label: 'Safe Mode',
      className: 'text-green-500 bg-green-500/10 border-green-500/30',
    },
    whitelist: {
      icon: ShieldAlert,
      label: 'Whitelist Mode',
      className: 'text-yellow-500 bg-yellow-500/10 border-yellow-500/30',
    },
    yolo: {
      icon: ShieldOff,
      label: 'YOLO Mode',
      className: 'text-red-500 bg-red-500/10 border-red-500/30',
    },
  };

  const { icon: Icon, label, className } = config[mode];

  return (
    <Badge variant="outline" className={cn('gap-1', className)}>
      <Icon className="h-3 w-3" />
      {label}
    </Badge>
  );
}

function SetupPrompt() {
  const navigateToSettings = () => {
    navigateTo('settings', 'agent');
  };

  return (
    <Card className="max-w-md mx-auto mt-8">
      <CardHeader>
        <div className="mx-auto w-12 h-12 rounded-full bg-primary/10 flex items-center justify-center mb-4">
          <Bot className="h-6 w-6 text-primary" />
        </div>
        <CardTitle className="text-center">Configure Agent</CardTitle>
        <CardDescription className="text-center">
          Set up your AI provider and API key to start using the agent.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <Button className="w-full" onClick={navigateToSettings}>
          Open Settings
        </Button>
      </CardContent>
    </Card>
  );
}

// =============================================================================
// Main Component
// =============================================================================

export function AgentPage() {
  const { settings } = useSettings();
  const { fadeInUp } = useAnimation();
  const session = useAgentSession();
  const [input, setInput] = useState('');
  const [showRightSidebar, setShowRightSidebar] = useState(true);

  const agentSettings = settings.agent;
  const isBusy = session.status !== 'idle';
  const isLoading = session.status === 'running';

  const toolSummary = useMemo(() => buildToolSummary(agentSettings), [agentSettings]);

  const handleSend = useCallback(async (text: string, attachments?: Parameters<typeof session.send>[1]) => {
    session.send(text, attachments);
  }, [session]);

  const handleRunInstrument = useCallback(
    (name: string) => session.send(`Please run the instrument: ${name}`),
    [session],
  );

  const { isConfigured, heartbeat: heartbeatStatus } = session;
  const stopAgentLoop = session.stop;

  // ==========================================================================
  // Render
  // ==========================================================================

  if (!isConfigured) {
    return (
      <motion.div {...fadeInUp} className="h-full flex items-center justify-center p-4">
        <SetupPrompt />
      </motion.div>
    );
  }

  return (
    <motion.div {...fadeInUp} className="h-full flex flex-col bg-background">
      {/* Header */}
      <header className="flex items-center justify-between p-3 border-b bg-background z-10">
        <div className="flex items-center gap-3">
          <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8 lg:hidden"
            onClick={() => setShowRightSidebar(!showRightSidebar)}
          >
            <Menu className="h-4 w-4" />
          </Button>
          <div className="w-8 h-8 rounded-full bg-primary/10 flex items-center justify-center">
            <Bot className="h-4 w-4 text-primary" />
          </div>
          <div>
            <h1 className="font-semibold text-sm flex items-center gap-2">
              ServiceAgent
              <Sparkles className="h-3 w-3 text-yellow-500" />
            </h1>
            <p className="text-[10px] text-muted-foreground leading-none">
              {agentSettings?.model || 'gpt-4o-mini'}
            </p>
          </div>
          <div className="hidden sm:block border-l pl-3 ml-1">
            <ConversationSelector
              currentConversationId={session.conversationId}
              onSelect={session.loadConversation}
              onNew={session.clear}
            />
          </div>
        </div>

        <div className="flex items-center gap-2">
          <div className="hidden md:block">
            <ApprovalModeIndicator mode={(agentSettings?.approvalMode || 'always') as ApprovalMode} />
          </div>
          {heartbeatStatus === 'stalled' && (
            <Badge
              variant="outline"
              className="gap-1.5 text-destructive bg-destructive/10 border-destructive/30 h-6"
            >
              <AlertTriangle className="h-3 w-3" />
              Stalled
            </Badge>
          )}
          {heartbeatStatus === 'stalled' && (
            <Button
              variant="outline"
              size="sm"
              className="h-7 text-chart-4 border-chart-4/30 hover:bg-chart-4/10"
              onClick={session.restart}
            >
              <RotateCcw className="h-3 w-3 mr-1" /> Force Restart
            </Button>
          )}
          {isLoading && heartbeatStatus !== 'stalled' && (
            <Badge
              variant="outline"
              className="gap-1.5 text-primary bg-primary/10 border-primary/30 animate-pulse h-6"
            >
              <Zap className="h-3 w-3" />
              Thinking...
            </Badge>
          )}
          {isBusy && (
            <Button
              variant="outline"
              size="sm"
              onClick={stopAgentLoop}
              className="h-7 text-destructive border-destructive/30 hover:bg-destructive/10"
            >
              <Square className="h-3 w-3 mr-1" /> Stop
            </Button>
          )}
          <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8 hidden lg:flex"
            onClick={() => setShowRightSidebar(!showRightSidebar)}
          >
            {showRightSidebar ? (
              <PanelRightClose className="h-4 w-4" />
            ) : (
              <PanelRight className="h-4 w-4" />
            )}
          </Button>
        </div>
      </header>

      {/* Main Layout */}
      <div className="flex-1 flex overflow-hidden">
        {/* Center Chat Area */}
        <div className="flex-1 flex flex-col min-w-0">
          <AgentMessageList
            messages={session.messages}
            isLoading={isLoading}
            activeServiceRun={session.activeServiceRun}
            onActivityApprove={session.approve}
            onActivityReject={session.reject}
            onSuggestionClick={setInput}
            onCancelServiceRun={session.dismissServiceRun}
          />

          <AgentInputArea
            input={input}
            onInputChange={setInput}
            isLoading={isBusy}
            awaitingApproval={session.status === 'awaiting_approval'}
            hasMessages={session.messages.length > 0}
            onSend={handleSend}
            onStop={stopAgentLoop}
            onClear={session.clear}
          />
        </div>

        {/* Right Sidebar */}
        <AnimatePresence initial={false}>
          {showRightSidebar && (
            <motion.div
              className="w-96 border-l bg-muted/10 shrink-0"
              initial={{ opacity: 0, x: 24 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: 24 }}
              transition={{ duration: 0.2, ease: [0.4, 0, 0.2, 1] }}
            >
              <AgentRightSidebar
                onRunInstrument={handleRunInstrument}
                mcpState={session.mcpState}
                toolSummary={toolSummary}
              />
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      {/* Mobile Drawer placeholder */}
      <div />
    </motion.div>
  );
}

export default AgentPage;
