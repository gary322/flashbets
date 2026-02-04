"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.MigrationWizard = void 0;
const react_1 = __importStar(require("react"));
const wallet_adapter_react_1 = require("@solana/wallet-adapter-react");
const framer_motion_1 = require("framer-motion");
const lucide_react_1 = require("lucide-react");
const MigrationWizard = ({ connection, programId, onComplete }) => {
    const { publicKey, connected } = (0, wallet_adapter_react_1.useWallet)();
    const [currentStep, setCurrentStep] = (0, react_1.useState)('welcome');
    const [positions, setPositions] = (0, react_1.useState)([]);
    const [scanning, setScanning] = (0, react_1.useState)(false);
    const [migrating, setMigrating] = (0, react_1.useState)(false);
    const [progress, setProgress] = (0, react_1.useState)(0);
    const [showAuditDetails, setShowAuditDetails] = (0, react_1.useState)(false);
    const [estimatedRewards, setEstimatedRewards] = (0, react_1.useState)(0);
    // Mock audit details for transparency
    const auditDetails = {
        auditor: 'Trail of Bits',
        date: new Date('2025-01-01'),
        findings: {
            critical: 0,
            high: 2,
            medium: 5,
            low: 12
        },
        resolved: true,
        reportUrl: 'ipfs://QmXoypizjW3WknFiJnKLwHCnL72vedxjQkDDP1mXWo6uco'
    };
    const steps = [
        { id: 'welcome', label: 'Welcome', icon: lucide_react_1.Info },
        { id: 'connect', label: 'Connect', icon: lucide_react_1.Shield },
        { id: 'scan', label: 'Scan', icon: lucide_react_1.CheckCircle },
        { id: 'review', label: 'Review', icon: lucide_react_1.TrendingUp },
        { id: 'confirm', label: 'Confirm', icon: lucide_react_1.AlertCircle },
        { id: 'processing', label: 'Process', icon: lucide_react_1.Clock },
        { id: 'complete', label: 'Complete', icon: lucide_react_1.Award }
    ];
    const scanPositions = () => __awaiter(void 0, void 0, void 0, function* () {
        setScanning(true);
        setProgress(0);
        // Simulate scanning with progress
        const interval = setInterval(() => {
            setProgress(prev => {
                if (prev >= 100) {
                    clearInterval(interval);
                    return 100;
                }
                return prev + 10;
            });
        }, 200);
        // Mock positions discovery
        setTimeout(() => {
            setPositions([
                {
                    id: '1',
                    market: 'Will BTC reach $100k by EOY?',
                    size: 1000,
                    value: 1250,
                    pnl: 250,
                    selected: true
                },
                {
                    id: '2',
                    market: '2024 Presidential Election',
                    size: 500,
                    value: 450,
                    pnl: -50,
                    selected: true
                },
                {
                    id: '3',
                    market: 'ETH Merge Success',
                    size: 2000,
                    value: 2400,
                    pnl: 400,
                    selected: true
                }
            ]);
            setScanning(false);
            setCurrentStep('review');
        }, 2000);
    });
    const calculateRewards = () => {
        const selectedValue = positions
            .filter(p => p.selected)
            .reduce((sum, p) => sum + p.value, 0);
        // 2x MMT rewards for migration
        const baseRewards = selectedValue * 0.01; // 1% base
        const migrationBonus = baseRewards * 2; // 2x multiplier
        setEstimatedRewards(migrationBonus);
    };
    (0, react_1.useEffect)(() => {
        if (positions.length > 0) {
            calculateRewards();
        }
    }, [positions]);
    const handleStepClick = (step) => {
        const stepOrder = ['welcome', 'connect', 'scan', 'review', 'confirm', 'processing', 'complete'];
        const currentIndex = stepOrder.indexOf(currentStep);
        const targetIndex = stepOrder.indexOf(step);
        if (targetIndex <= currentIndex) {
            setCurrentStep(step);
        }
    };
    const renderStepContent = () => {
        switch (currentStep) {
            case 'welcome':
                return (<framer_motion_1.motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} className="space-y-6">
            <div className="text-center">
              <h2 className="text-3xl font-bold mb-4">Safe Migration Wizard</h2>
              <p className="text-gray-600 dark:text-gray-300 mb-6">
                Migrate your positions to the new platform and earn double MMT rewards
              </p>
            </div>
            
            <div className="bg-blue-50 dark:bg-blue-900/20 p-6 rounded-lg">
              <h3 className="font-semibold mb-2 flex items-center">
                <lucide_react_1.Award className="w-5 h-5 mr-2 text-blue-600"/>
                Migration Benefits
              </h3>
              <ul className="space-y-2 text-sm">
                <li>• 2x MMT rewards on all migrated positions</li>
                <li>• Priority access to new features</li>
                <li>• Lower fees for 60 days</li>
                <li>• Automatic position optimization</li>
              </ul>
            </div>

            <button onClick={() => setCurrentStep('connect')} className="w-full bg-blue-600 hover:bg-blue-700 text-white py-3 rounded-lg font-semibold transition-colors">
              Start Migration
            </button>

            <button onClick={() => setShowAuditDetails(!showAuditDetails)} className="w-full text-sm text-gray-500 hover:text-gray-700 transition-colors">
              {showAuditDetails ? 'Hide' : 'Show'} Audit Details
            </button>

            <framer_motion_1.AnimatePresence>
              {showAuditDetails && (<framer_motion_1.motion.div initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }} className="bg-gray-50 dark:bg-gray-800 p-4 rounded-lg">
                  <h4 className="font-semibold mb-2">Security Audit</h4>
                  <div className="text-sm space-y-1">
                    <p>Auditor: {auditDetails.auditor}</p>
                    <p>Date: {auditDetails.date.toLocaleDateString()}</p>
                    <p>Critical: {auditDetails.findings.critical} | 
                       High: {auditDetails.findings.high} | 
                       Medium: {auditDetails.findings.medium} | 
                       Low: {auditDetails.findings.low}</p>
                    <p className="text-green-600">✓ All findings resolved</p>
                    <a href={auditDetails.reportUrl} className="text-blue-600 hover:underline" target="_blank" rel="noopener noreferrer">
                      View Full Report
                    </a>
                  </div>
                </framer_motion_1.motion.div>)}
            </framer_motion_1.AnimatePresence>
          </framer_motion_1.motion.div>);
            case 'connect':
                return (<framer_motion_1.motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} className="space-y-6">
            <div className="text-center">
              <h2 className="text-2xl font-bold mb-4">Connect Your Wallet</h2>
              <p className="text-gray-600 dark:text-gray-300">
                Connect your wallet to scan for existing positions
              </p>
            </div>

            {connected ? (<div className="space-y-4">
                <div className="bg-green-50 dark:bg-green-900/20 p-4 rounded-lg">
                  <p className="text-green-800 dark:text-green-200 flex items-center">
                    <lucide_react_1.CheckCircle className="w-5 h-5 mr-2"/>
                    Wallet connected: {publicKey === null || publicKey === void 0 ? void 0 : publicKey.toBase58().slice(0, 8)}...
                  </p>
                </div>
                <button onClick={() => setCurrentStep('scan')} className="w-full bg-blue-600 hover:bg-blue-700 text-white py-3 rounded-lg font-semibold">
                  Continue to Scan
                </button>
              </div>) : (<div className="text-center py-8">
                <p className="text-gray-500 mb-4">Please connect your wallet to continue</p>
                {/* Wallet connect button would go here */}
              </div>)}
          </framer_motion_1.motion.div>);
            case 'scan':
                return (<framer_motion_1.motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} className="space-y-6">
            <div className="text-center">
              <h2 className="text-2xl font-bold mb-4">Scanning Positions</h2>
              <p className="text-gray-600 dark:text-gray-300">
                Looking for your existing positions to migrate
              </p>
            </div>

            {scanning ? (<div className="space-y-4">
                <div className="relative pt-1">
                  <div className="flex mb-2 items-center justify-between">
                    <div>
                      <span className="text-xs font-semibold inline-block py-1 px-2 uppercase rounded-full text-blue-600 bg-blue-200">
                        Scanning
                      </span>
                    </div>
                    <div className="text-right">
                      <span className="text-xs font-semibold inline-block text-blue-600">
                        {progress}%
                      </span>
                    </div>
                  </div>
                  <div className="overflow-hidden h-2 mb-4 text-xs flex rounded bg-blue-200">
                    <framer_motion_1.motion.div initial={{ width: 0 }} animate={{ width: `${progress}%` }} className="shadow-none flex flex-col text-center whitespace-nowrap text-white justify-center bg-blue-600"/>
                  </div>
                </div>
              </div>) : (<button onClick={scanPositions} className="w-full bg-blue-600 hover:bg-blue-700 text-white py-3 rounded-lg font-semibold">
                Start Scanning
              </button>)}
          </framer_motion_1.motion.div>);
            case 'review':
                return (<framer_motion_1.motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} className="space-y-6">
            <div className="text-center">
              <h2 className="text-2xl font-bold mb-4">Review Positions</h2>
              <p className="text-gray-600 dark:text-gray-300">
                Select which positions to migrate
              </p>
            </div>

            <div className="space-y-3">
              {positions.map(position => (<div key={position.id} className="bg-white dark:bg-gray-800 p-4 rounded-lg border border-gray-200 dark:border-gray-700">
                  <div className="flex items-center justify-between">
                    <div className="flex-1">
                      <h4 className="font-semibold">{position.market}</h4>
                      <div className="flex items-center gap-4 text-sm text-gray-600 dark:text-gray-300 mt-1">
                        <span>Size: ${position.size}</span>
                        <span>Value: ${position.value}</span>
                        <span className={position.pnl >= 0 ? 'text-green-600' : 'text-red-600'}>
                          PnL: {position.pnl >= 0 ? '+' : ''}{position.pnl}
                        </span>
                      </div>
                    </div>
                    <input type="checkbox" checked={position.selected} onChange={(e) => {
                            setPositions(positions.map(p => p.id === position.id
                                ? Object.assign(Object.assign({}, p), { selected: e.target.checked }) : p));
                        }} className="w-5 h-5"/>
                  </div>
                </div>))}
            </div>

            <div className="bg-gradient-to-r from-blue-50 to-purple-50 dark:from-blue-900/20 dark:to-purple-900/20 p-6 rounded-lg">
              <h3 className="font-semibold mb-2 flex items-center">
                <lucide_react_1.DollarSign className="w-5 h-5 mr-2 text-purple-600"/>
                Estimated Rewards
              </h3>
              <p className="text-3xl font-bold text-purple-600">
                {estimatedRewards.toFixed(2)} MMT
              </p>
              <p className="text-sm text-gray-600 dark:text-gray-300 mt-1">
                2x bonus for migration applied
              </p>
            </div>

            <div className="flex gap-3">
              <button onClick={() => {
                        setPositions(positions.map(p => (Object.assign(Object.assign({}, p), { selected: true }))));
                    }} className="flex-1 bg-gray-200 hover:bg-gray-300 dark:bg-gray-700 dark:hover:bg-gray-600 text-gray-800 dark:text-gray-200 py-3 rounded-lg font-semibold">
                Select All
              </button>
              <button onClick={() => setCurrentStep('confirm')} disabled={!positions.some(p => p.selected)} className="flex-1 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-400 text-white py-3 rounded-lg font-semibold">
                Continue
              </button>
            </div>
          </framer_motion_1.motion.div>);
            case 'confirm':
                const selectedPositions = positions.filter(p => p.selected);
                const totalValue = selectedPositions.reduce((sum, p) => sum + p.value, 0);
                return (<framer_motion_1.motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} className="space-y-6">
            <div className="text-center">
              <h2 className="text-2xl font-bold mb-4">Confirm Migration</h2>
              <p className="text-gray-600 dark:text-gray-300">
                Review and confirm your migration details
              </p>
            </div>

            <div className="bg-yellow-50 dark:bg-yellow-900/20 p-4 rounded-lg">
              <p className="text-yellow-800 dark:text-yellow-200 flex items-center text-sm">
                <lucide_react_1.AlertCircle className="w-5 h-5 mr-2 flex-shrink-0"/>
                Migration is permanent. Your positions will be moved to the new platform.
              </p>
            </div>

            <div className="space-y-4">
              <div className="bg-gray-50 dark:bg-gray-800 p-4 rounded-lg">
                <h4 className="font-semibold mb-2">Migration Summary</h4>
                <div className="space-y-2 text-sm">
                  <div className="flex justify-between">
                    <span>Positions to migrate:</span>
                    <span className="font-semibold">{selectedPositions.length}</span>
                  </div>
                  <div className="flex justify-between">
                    <span>Total value:</span>
                    <span className="font-semibold">${totalValue}</span>
                  </div>
                  <div className="flex justify-between">
                    <span>Estimated rewards:</span>
                    <span className="font-semibold text-purple-600">
                      {estimatedRewards.toFixed(2)} MMT
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span>Estimated gas:</span>
                    <span className="font-semibold">~0.05 SOL</span>
                  </div>
                </div>
              </div>
            </div>

            <div className="flex gap-3">
              <button onClick={() => setCurrentStep('review')} className="flex-1 bg-gray-200 hover:bg-gray-300 dark:bg-gray-700 dark:hover:bg-gray-600 text-gray-800 dark:text-gray-200 py-3 rounded-lg font-semibold">
                Back
              </button>
              <button onClick={() => {
                        setCurrentStep('processing');
                        // Start migration process
                        setTimeout(() => {
                            setCurrentStep('complete');
                        }, 3000);
                    }} className="flex-1 bg-green-600 hover:bg-green-700 text-white py-3 rounded-lg font-semibold">
                Confirm Migration
              </button>
            </div>
          </framer_motion_1.motion.div>);
            case 'processing':
                return (<framer_motion_1.motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} className="space-y-6">
            <div className="text-center">
              <h2 className="text-2xl font-bold mb-4">Processing Migration</h2>
              <p className="text-gray-600 dark:text-gray-300">
                Please wait while we migrate your positions
              </p>
            </div>

            <div className="flex justify-center py-8">
              <div className="animate-spin rounded-full h-16 w-16 border-b-2 border-blue-600"></div>
            </div>

            <p className="text-center text-sm text-gray-500">
              This may take a few moments...
            </p>
          </framer_motion_1.motion.div>);
            case 'complete':
                return (<framer_motion_1.motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} className="space-y-6">
            <div className="text-center">
              <framer_motion_1.motion.div initial={{ scale: 0 }} animate={{ scale: 1 }} transition={{ type: "spring", stiffness: 200 }} className="w-20 h-20 bg-green-100 dark:bg-green-900/20 rounded-full flex items-center justify-center mx-auto mb-4">
                <lucide_react_1.CheckCircle className="w-10 h-10 text-green-600"/>
              </framer_motion_1.motion.div>
              <h2 className="text-2xl font-bold mb-4">Migration Complete!</h2>
              <p className="text-gray-600 dark:text-gray-300">
                Your positions have been successfully migrated
              </p>
            </div>

            <div className="bg-green-50 dark:bg-green-900/20 p-6 rounded-lg">
              <h3 className="font-semibold mb-2">Migration Results</h3>
              <div className="space-y-2 text-sm">
                <p>✓ {positions.filter(p => p.selected).length} positions migrated</p>
                <p>✓ {estimatedRewards.toFixed(2)} MMT rewards earned</p>
                <p>✓ Double rewards activated for 60 days</p>
              </div>
            </div>

            <div className="flex gap-3">
              <button onClick={() => {
                        // View positions
                    }} className="flex-1 bg-blue-600 hover:bg-blue-700 text-white py-3 rounded-lg font-semibold">
                View New Positions
              </button>
              <button onClick={() => {
                        onComplete === null || onComplete === void 0 ? void 0 : onComplete();
                    }} className="flex-1 bg-gray-200 hover:bg-gray-300 dark:bg-gray-700 dark:hover:bg-gray-600 text-gray-800 dark:text-gray-200 py-3 rounded-lg font-semibold">
                Close
              </button>
            </div>
          </framer_motion_1.motion.div>);
        }
    };
    return (<div className="max-w-2xl mx-auto p-6">
      {/* Progress Steps */}
      <div className="mb-8">
        <div className="flex items-center justify-between">
          {steps.map((step, index) => {
            const Icon = step.icon;
            const isActive = step.id === currentStep;
            const isCompleted = steps.findIndex(s => s.id === currentStep) > index;
            return (<div key={step.id} className="flex items-center">
                <button onClick={() => handleStepClick(step.id)} className={`
                    relative flex items-center justify-center w-10 h-10 rounded-full
                    ${isActive ? 'bg-blue-600 text-white' :
                    isCompleted ? 'bg-green-600 text-white' :
                        'bg-gray-200 dark:bg-gray-700 text-gray-400'}
                    transition-colors cursor-pointer
                  `}>
                  <Icon className="w-5 h-5"/>
                </button>
                {index < steps.length - 1 && (<div className={`
                    w-full h-1 mx-2
                    ${isCompleted ? 'bg-green-600' : 'bg-gray-200 dark:bg-gray-700'}
                    transition-colors
                  `}/>)}
              </div>);
        })}
        </div>
        <div className="flex justify-between mt-2">
          {steps.map(step => (<span key={step.id} className="text-xs text-gray-600 dark:text-gray-400">
              {step.label}
            </span>))}
        </div>
      </div>

      {/* Step Content */}
      <div className="bg-white dark:bg-gray-900 rounded-xl shadow-lg p-6">
        {renderStepContent()}
      </div>
    </div>);
};
exports.MigrationWizard = MigrationWizard;
