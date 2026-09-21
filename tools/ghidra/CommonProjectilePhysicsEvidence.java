import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.mem.Memory;

public class CommonProjectilePhysicsEvidence extends GhidraScript {
    private void dump(long raw, DecompInterface d) throws Exception {
        Address a=toAddr(raw); println(String.format("\n=== 0x%08X ===",raw));
        Function f=getFunctionContaining(a);
        if(f==null){ println("NO_FUNCTION"); return; }
        println("FUNCTION="+f.getName()+" ENTRY="+f.getEntryPoint());
        DecompileResults r=d.decompileFunction(f,60,monitor);
        if(r.decompileCompleted()&&r.getDecompiledFunction()!=null) println(r.getDecompiledFunction().getC());
    }
    @Override public void run() throws Exception {
        DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
        Memory m=currentProgram.getMemory();
        long vt=0x005DF390L;
        for(long off:new long[]{0xCCL,0xD0L,0xD4L,0xD8L}) {
            long target=m.getInt(toAddr(vt+off)) & 0xffffffffL;
            println(String.format("MINE_VTABLE +0x%02X -> 0x%08X",off,target));
        }
        for(long a:new long[]{0x0041D5C0L,0x0041E960L,0x0041EB10L,0x0041EFF0L,0x00414810L,0x004148B0L,0x004149A0L,0x00414EF0L,0x004150E0L,0x00415250L,0x00415260L,0x004152A0L,0x00425800L,0x004258E0L,0x00425A70L,0x00425C70L}) dump(a,d);
        d.dispose();
    }
}
