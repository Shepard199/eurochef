import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.data.DataType;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.Symbol;

public class PiranhaDetailEvidence extends GhidraScript {
    private void dump(long raw, int count) throws Exception {
        Address a=toAddr(raw); disassemble(a);
        println(String.format("\n=== LINEAR 0x%08X ===",raw));
        InstructionIterator it=currentProgram.getListing().getInstructions(a,true);
        int n=0;
        while(it.hasNext()&&n<count){
            Instruction ins=it.next();
            println(ins.getAddress()+"  "+ins);
            for(Reference ref:ins.getReferencesFrom()) println("    REF "+ref.getReferenceType()+" -> "+ref.getToAddress());
            n++;
        }
    }
    private void words(long raw,int count)throws Exception{
        println(String.format("\n=== WORDS 0x%08X ===",raw));
        for(int i=0;i<count;i++){
            long a=raw+i*4L;
            long v=Integer.toUnsignedLong(getInt(toAddr(a)));
            Symbol s=getSymbolAt(toAddr(v));
            println(String.format("+%02X 0x%08X%s",i*4,v,s==null?"":" "+s.getName()));
        }
    }
    @Override public void run() throws Exception {
        dump(0x00467940L, 32);
        dump(0x00467EC0L, 90);
        words(0x005EBDF0L,16);
        long[] cs={0x005DD3C8L,0x005DFF08L,0x005DDEA4L,0x005DD3BCL,0x005DD3C4L,0x005DE538L};
        for(long a:cs){int raw=getInt(toAddr(a)); println(String.format("CONST 0x%08X raw=0x%08X f=%s",a,raw,Float.intBitsToFloat(raw)));}
    }
}
