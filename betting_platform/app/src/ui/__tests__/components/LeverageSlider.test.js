"use strict";
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
const react_1 = __importDefault(require("react"));
const test_utils_1 = require("../test-utils");
const LeverageSlider_1 = require("../../components/trading/LeverageSlider");
describe('LeverageSlider', () => {
    const defaultProps = {
        value: 10,
        onChange: jest.fn(),
        max: 100,
        effectiveLeverage: 10,
        showWarnings: true,
        coverage: 1.5
    };
    beforeEach(() => {
        defaultProps.onChange.mockClear();
    });
    describe('Rendering', () => {
        it('should render with correct initial values', () => {
            const { getByText } = (0, test_utils_1.render)(<LeverageSlider_1.LeverageSlider {...defaultProps}/>);
            expect(getByText('10')).toBeInTheDocument();
            expect(getByText('Base Leverage')).toBeInTheDocument();
        });
        it('should display effective leverage when different from base', () => {
            const { getByText } = (0, test_utils_1.render)(<LeverageSlider_1.LeverageSlider {...defaultProps} effectiveLeverage={25}/>);
            expect(getByText('25.0x')).toBeInTheDocument();
            expect(getByText('Effective (with chain)')).toBeInTheDocument();
        });
        it('should not display effective leverage when same as base', () => {
            const { queryByText } = (0, test_utils_1.render)(<LeverageSlider_1.LeverageSlider {...defaultProps}/>);
            expect(queryByText('Effective (with chain)')).not.toBeInTheDocument();
        });
    });
    describe('Preset Buttons', () => {
        it('should render all preset buttons', () => {
            const { getByText } = (0, test_utils_1.render)(<LeverageSlider_1.LeverageSlider {...defaultProps}/>);
            expect(getByText('1x')).toBeInTheDocument();
            expect(getByText('10x')).toBeInTheDocument();
            expect(getByText('25x')).toBeInTheDocument();
            expect(getByText('50x')).toBeInTheDocument();
            expect(getByText('100x')).toBeInTheDocument();
            expect(getByText('MAX')).toBeInTheDocument();
        });
        it('should call onChange when preset button is clicked', () => {
            const { getByText } = (0, test_utils_1.render)(<LeverageSlider_1.LeverageSlider {...defaultProps}/>);
            test_utils_1.fireEvent.click(getByText('50x'));
            expect(defaultProps.onChange).toHaveBeenCalledWith(50);
        });
        it('should highlight active preset button', () => {
            const { getByText } = (0, test_utils_1.render)(<LeverageSlider_1.LeverageSlider {...defaultProps} value={25}/>);
            const button = getByText('25x');
            expect(button).toHaveStyle('background: #00FF88');
        });
        it('should set max value when MAX button is clicked', () => {
            const { getByText } = (0, test_utils_1.render)(<LeverageSlider_1.LeverageSlider {...defaultProps}/>);
            test_utils_1.fireEvent.click(getByText('MAX'));
            expect(defaultProps.onChange).toHaveBeenCalledWith(100);
        });
    });
    describe('Warning Messages', () => {
        it('should not show warning for safe leverage', () => {
            const { queryByText } = (0, test_utils_1.render)(<LeverageSlider_1.LeverageSlider {...defaultProps} effectiveLeverage={40}/>);
            expect(queryByText(/LEVERAGE/)).not.toBeInTheDocument();
        });
        it('should show moderate warning at 50x+', () => {
            const { getByText } = (0, test_utils_1.render)(<LeverageSlider_1.LeverageSlider {...defaultProps} effectiveLeverage={75}/>);
            expect(getByText(/Moderate leverage/)).toBeInTheDocument();
            expect(getByText(/Liquidation buffer: 1.33%/)).toBeInTheDocument();
        });
        it('should show high leverage warning at 100x+', () => {
            const { getByText } = (0, test_utils_1.render)(<LeverageSlider_1.LeverageSlider {...defaultProps} effectiveLeverage={150}/>);
            expect(getByText(/HIGH LEVERAGE/)).toBeInTheDocument();
            expect(getByText('150.0x effective')).toBeInTheDocument();
            expect(getByText(/Liquidation on 0.67% move/)).toBeInTheDocument();
        });
        it('should show extreme warning at 300x+', () => {
            const { getByText } = (0, test_utils_1.render)(<LeverageSlider_1.LeverageSlider {...defaultProps} effectiveLeverage={450}/>);
            expect(getByText(/EXTREME LEVERAGE/)).toBeInTheDocument();
            expect(getByText('450.0x effective')).toBeInTheDocument();
            expect(getByText(/You will be liquidated on a 0.22% adverse move/)).toBeInTheDocument();
        });
        it('should not show warnings when showWarnings is false', () => {
            const { queryByText } = (0, test_utils_1.render)(<LeverageSlider_1.LeverageSlider {...defaultProps} effectiveLeverage={150} showWarnings={false}/>);
            expect(queryByText(/HIGH LEVERAGE/)).not.toBeInTheDocument();
        });
    });
    describe('Slider Interaction', () => {
        it('should update value when track is clicked', () => {
            const { container } = (0, test_utils_1.render)(<LeverageSlider_1.LeverageSlider {...defaultProps}/>);
            const track = container.querySelector('[data-testid="slider-track"]') ||
                container.querySelector('div[style*="cursor: pointer"]');
            if (track) {
                // Mock getBoundingClientRect
                Object.defineProperty(track, 'getBoundingClientRect', {
                    value: () => ({
                        left: 0,
                        width: 200,
                        top: 0,
                        height: 8
                    })
                });
                test_utils_1.fireEvent.click(track, { clientX: 100 }); // Click at middle
                expect(defaultProps.onChange).toHaveBeenCalledWith(50);
            }
        });
        it('should handle drag interaction on thumb', () => __awaiter(void 0, void 0, void 0, function* () {
            const { container } = (0, test_utils_1.render)(<LeverageSlider_1.LeverageSlider {...defaultProps}/>);
            const thumb = container.querySelector('[drag="x"]');
            if (thumb) {
                // Mock parent element
                Object.defineProperty(thumb, 'parentElement', {
                    value: {
                        getBoundingClientRect: () => ({
                            left: 0,
                            width: 200,
                            top: 0,
                            height: 8
                        })
                    }
                });
                // Simulate drag
                test_utils_1.fireEvent.mouseDown(thumb);
                test_utils_1.fireEvent.mouseMove(thumb, { clientX: 160 });
                test_utils_1.fireEvent.mouseUp(thumb);
                yield (0, test_utils_1.waitFor)(() => {
                    expect(defaultProps.onChange).toHaveBeenCalled();
                });
            }
        }));
    });
    describe('Visual States', () => {
        it('should change track color based on warning level', () => {
            const { container, rerender } = (0, test_utils_1.render)(<LeverageSlider_1.LeverageSlider {...defaultProps} effectiveLeverage={40}/>);
            let track = container.querySelector('[danger="false"]');
            expect(track).toBeInTheDocument();
            rerender(<LeverageSlider_1.LeverageSlider {...defaultProps} effectiveLeverage={150}/>);
            track = container.querySelector('[danger="true"]');
            expect(track).toBeInTheDocument();
            rerender(<LeverageSlider_1.LeverageSlider {...defaultProps} effectiveLeverage={350}/>);
            track = container.querySelector('[extreme="true"]');
            expect(track).toBeInTheDocument();
        });
        it('should change thumb color in danger zone', () => {
            const { container } = (0, test_utils_1.render)(<LeverageSlider_1.LeverageSlider {...defaultProps} effectiveLeverage={150}/>);
            const thumb = container.querySelector('[danger="true"]');
            expect(thumb).toBeInTheDocument();
            expect(thumb).toHaveStyle('background: #DC2626');
        });
    });
    describe('Liquidation Calculations', () => {
        it('should calculate liquidation buffer correctly', () => {
            const testCases = [
                { leverage: 10, expectedBuffer: '10.00%' },
                { leverage: 50, expectedBuffer: '2.00%' },
                { leverage: 100, expectedBuffer: '1.00%' },
                { leverage: 500, expectedBuffer: '0.20%' }
            ];
            testCases.forEach(({ leverage, expectedBuffer }) => {
                const { getByText } = (0, test_utils_1.render)(<LeverageSlider_1.LeverageSlider {...defaultProps} effectiveLeverage={leverage}/>);
                if (leverage >= 50) {
                    expect(getByText(new RegExp(expectedBuffer))).toBeInTheDocument();
                }
            });
        });
    });
    describe('Animations', () => {
        it('should animate warning message appearance', () => __awaiter(void 0, void 0, void 0, function* () {
            const { container, rerender } = (0, test_utils_1.render)(<LeverageSlider_1.LeverageSlider {...defaultProps} effectiveLeverage={40}/>);
            expect(container.querySelector('[initial]')).not.toBeInTheDocument();
            rerender(<LeverageSlider_1.LeverageSlider {...defaultProps} effectiveLeverage={150}/>);
            const warningMessage = container.querySelector('[initial]');
            expect(warningMessage).toBeInTheDocument();
            expect(warningMessage).toHaveAttribute('animate');
        }));
    });
    describe('Edge Cases', () => {
        it('should handle zero leverage', () => {
            const { container } = (0, test_utils_1.render)(<LeverageSlider_1.LeverageSlider {...defaultProps} value={0} effectiveLeverage={0}/>);
            expect(container).toBeInTheDocument();
        });
        it('should handle maximum leverage', () => {
            const { getByText } = (0, test_utils_1.render)(<LeverageSlider_1.LeverageSlider {...defaultProps} value={100} effectiveLeverage={100}/>);
            expect(getByText('100')).toBeInTheDocument();
        });
        it('should handle undefined coverage gracefully', () => {
            const { container } = (0, test_utils_1.render)(<LeverageSlider_1.LeverageSlider {...defaultProps} coverage={undefined}/>);
            expect(container).toBeInTheDocument();
        });
    });
});
