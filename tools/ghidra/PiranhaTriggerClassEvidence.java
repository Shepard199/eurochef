import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolIterator;

public class PiranhaTriggerClassEvidence extends GhidraScript {
    private Function ensure(long raw) throws Exception {
        Address a=toAddr(raw);
        Function f=getFunctionAt(a);
        if(f==null){disassemble(a);try{f=createFunction(a,null);}catch(Exception ignored){}}
        if(f==null)f=getFunctionContaining(a);
        return f;
    }
    private void dump(long raw, DecompInterface d)throws Exception{
        Function f=ensure(raw);
        println(String.format("\n=== 0x%08X %s ===",raw,f==null?"<missing>":f.getName()));
        if(f==null)return;
        DecompileResults r=d.decompileFunction(f,90,monitor);
        if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
    }
    private void refsToString(String needle)throws Exception{
        println("\n=== string refs "+needle+" ===");
        SymbolIterator it=currentProgram.getSymbolTable().getAllSymbols(true);
        while(it.hasNext()){
            Symbol s=it.next();
            if(s.getName().contains(needle)){
                println("SYM "+s.getAddress()+" "+s.getName());
                for(Reference ref:getReferencesTo(s.getAddress())) println("  REF "+ref.getFromAddress()+" "+ref.getReferenceType());
            }
        }
    }
    @Override public void run() throws Exception{
        DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
        dump(0x0048CC30L,d);
        dump(0x0048CCA0L,d);
        refsToString("XTrigger_Monster_Fish");
        refsToString("XTrigger_Distance");
        d.dispose();
    }
}
