import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;

public class TransporterHandlerEvidence extends GhidraScript {
    private final DecompInterface decompiler = new DecompInterface();

    private void decompileAt(long raw) throws Exception {
        Address address = toAddr(raw);
        Function function = getFunctionContaining(address);
        println(String.format("\n=== TARGET 0x%08X ===", raw));
        if (function == null) {
            println("NO_FUNCTION");
            return;
        }
        println("FUNCTION=" + function.getName() + " ENTRY=" + function.getEntryPoint());
        DecompileResults results = decompiler.decompileFunction(function, 60, monitor);
        if (results.decompileCompleted() && results.getDecompiledFunction() != null) {
            println(results.getDecompiledFunction().getC());
        } else {
            println("DECOMPILE_FAILED=" + results.getErrorMessage());
        }
    }

    private void callersOf(long raw) throws Exception {
        Address target = toAddr(raw);
        println(String.format("\n=== CALLERS 0x%08X ===", raw));
        ReferenceIterator references = currentProgram.getReferenceManager().getReferencesTo(target);
        int count = 0;
        while (references.hasNext()) {
            Reference reference = references.next();
            if (!reference.getReferenceType().isCall()) {
                continue;
            }
            Function caller = getFunctionContaining(reference.getFromAddress());
            println("CALL " + reference.getFromAddress() + " FROM "
                + (caller == null ? "<none>" : caller.getName() + "@" + caller.getEntryPoint()));
            count++;
        }
        println("CALLER_COUNT=" + count);
    }

    private void disassembleAt(long raw, int maxInstructions) throws Exception {
        println(String.format("\n=== DISASM 0x%08X ===", raw));
        Instruction instruction = getInstructionAt(toAddr(raw));
        int seen = 0;
        while (instruction != null && seen < maxInstructions) {
            println(instruction.getAddress() + " " + instruction);
            if (instruction.getMnemonicString().equalsIgnoreCase("RET")) {
                break;
            }
            instruction = instruction.getNext();
            seen++;
        }
    }

    private void callRefsFrom(long raw, int maxInstructions) throws Exception {
        Address address = toAddr(raw);
        Function function = getFunctionContaining(address);
        if (function == null) {
            return;
        }
        println(String.format("\n=== CALL REFS FROM 0x%08X ===", raw));
        InstructionIterator it = currentProgram.getListing().getInstructions(function.getBody(), true);
        int seen = 0;
        while (it.hasNext() && seen < maxInstructions) {
            Instruction instruction = it.next();
            for (Reference reference : instruction.getReferencesFrom()) {
                if (reference.getReferenceType().isCall()) {
                    println(instruction.getAddress() + " " + instruction + " -> " + reference.getToAddress());
                }
            }
            seen++;
        }
    }

    @Override
    public void run() throws Exception {
        println("PROGRAM=" + currentProgram.getName());
        println(String.format("NODE_ARRIVAL_RADIUS_SQ=%f bits=0x%08X", getFloat(toAddr(0x005DD3B8L)), getInt(toAddr(0x005DD3B8L))));
        long monsterVtable = 0x005E82D0L;
        long monsterCreate = Integer.toUnsignedLong(getInt(toAddr(monsterVtable + 0x24)));
        long monsterOwnedXItem = Integer.toUnsignedLong(getInt(toAddr(monsterVtable + 0x80)));
        long transporterVtable = 0x005E8608L;
        long transporterCreate = Integer.toUnsignedLong(getInt(toAddr(transporterVtable + 0x24)));
        println(String.format("XTRIGGER_MONSTER_VTABLE=0x%08X CREATE_SLOT_24=0x%08X OWNED_XITEM_SLOT_80=0x%08X", monsterVtable, monsterCreate, monsterOwnedXItem));
        println(String.format("XTRIGGER_TRANSPORTER_VTABLE=0x%08X CREATE_SLOT_24=0x%08X", transporterVtable, transporterCreate));
        disassembleAt(monsterOwnedXItem, 8);
        decompiler.openProgram(currentProgram);
        long[] targets = {
            transporterCreate,
            monsterCreate,
            monsterOwnedXItem,
            0x00469020L,
            0x00469660L,
            0x00469350L,
            0x00469450L,
            0x00420890L,
            0x00420B40L,
            0x0047FE20L,
            0x0044BBB0L,
            0x0047F910L
        };
        for (long target : targets) {
            callersOf(target);
            decompileAt(target);
            callRefsFrom(target, 400);
        }
        decompiler.dispose();
    }
}
